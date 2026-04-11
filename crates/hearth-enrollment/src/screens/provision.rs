use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{List, ListItem, Paragraph};
use std::time::Instant;
use tracing::{error, info, warn};

use crate::app::EnrollmentData;
use crate::ui;

use hearth_common::api_client::{HearthApiClient, ReqwestApiClient};

#[derive(Debug)]
struct BlockDevice {
    name: String,
    size: String,
    tran: String,
}

#[derive(Debug)]
enum ProvisionState {
    WaitingForClosure,
    SelectDisk {
        devices: Vec<BlockDevice>,
        selected: usize,
    },
    RunningDisko,
    Partitioning,
    Formatting,
    Mounting,
    Installing {
        progress: String,
    },
    Complete,
    Rebooting,
    Error {
        step: String,
        message: String,
    },
}

pub struct ProvisionScreen {
    state: ProvisionState,
    client: Option<ReqwestApiClient>,
    machine_id: Option<uuid::Uuid>,
    target_closure: Option<String>,
    cache_url: Option<String>,
    cache_token: Option<String>,
    machine_token: Option<String>,
    target_disk: Option<String>,
    /// Disko config name (e.g., "standard", "luks-lvm"). When set, disko is used
    /// instead of manual sgdisk/mkfs partitioning.
    disko_config: Option<String>,
    /// Headscale pre-auth key for mesh VPN join on first boot.
    headscale_preauth_key: Option<String>,
    last_poll: Option<Instant>,
    dots: usize,
    log_lines: Vec<String>,
    started: bool,
}

impl ProvisionScreen {
    pub fn new() -> Self {
        Self {
            state: ProvisionState::WaitingForClosure,
            client: None,
            machine_id: None,
            target_closure: None,
            cache_url: None,
            cache_token: None,
            machine_token: None,
            target_disk: None,
            disko_config: None,
            headscale_preauth_key: None,
            last_poll: None,
            dots: 0,
            log_lines: Vec::new(),
            started: false,
        }
    }

    /// Initialize the screen with enrollment data from previous steps.
    pub fn start(&mut self, data: &EnrollmentData) {
        if self.started {
            return;
        }
        self.started = true;
        // Use the machine token (if available) for authenticated API calls
        // (e.g., fetching fresh cache tokens). Fall back to user token or no auth.
        self.client = Some(match (&data.machine_token, &data.user_token) {
            (Some(token), _) => ReqwestApiClient::new_with_token(data.server_url.clone(), token.clone()),
            (_, Some(token)) => ReqwestApiClient::new_with_token(data.server_url.clone(), token.clone()),
            _ => ReqwestApiClient::new(data.server_url.clone()),
        });
        self.machine_id = data.machine_id;
        // If the enrollment data already has a target closure (set during approval),
        // use it directly.
        if let Some(ref closure) = data.target_closure {
            self.target_closure = Some(closure.clone());
        }
        self.cache_url = data.cache_url.clone();
        self.cache_token = data.cache_token.clone();
        self.machine_token = data.machine_token.clone();
        self.disko_config = data.disko_config.clone();
        self.headscale_preauth_key = data.headscale_preauth_key.clone();
        info!(
            machine_id = ?self.machine_id,
            has_cache_token = self.cache_token.is_some(),
            disko_config = ?self.disko_config,
            has_headscale_key = self.headscale_preauth_key.is_some(),
            "provisioning screen started"
        );
    }

    /// Returns the partition suffix for a given device name.
    /// NVMe and MMC devices use "p" prefix before partition number,
    /// while SATA/virtio drives use just the number.
    fn partition_suffix(device_name: &str) -> &'static str {
        if device_name.contains("nvme") || device_name.contains("mmc") {
            "p"
        } else {
            ""
        }
    }

    /// Build partition device path, e.g. /dev/nvme0n1p1 or /dev/sda1
    fn partition_path(disk: &str, part_num: u8) -> String {
        let suffix = Self::partition_suffix(disk);
        format!("/dev/{disk}{suffix}{part_num}")
    }

    fn log(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        info!("{}", msg);
        self.log_lines.push(msg);
        // Keep a reasonable scrollback
        if self.log_lines.len() > 200 {
            self.log_lines.remove(0);
        }
    }

    /// Run a shell command, returning Ok(stdout) or Err(message).
    async fn run_cmd(cmd: &str, args: &[&str]) -> Result<String, String> {
        info!(cmd = cmd, args = ?args, "running command");
        match tokio::process::Command::new(cmd).args(args).output().await {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                if output.status.success() {
                    Ok(stdout)
                } else {
                    let code = output
                        .status
                        .code()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "signal".into());
                    Err(format!(
                        "{cmd} exited with {code}\nstdout: {stdout}\nstderr: {stderr}"
                    ))
                }
            }
            Err(e) => Err(format!("Failed to execute {cmd}: {e}")),
        }
    }

    /// Discover block devices suitable for installation.
    async fn discover_disks(&mut self) {
        self.log("Scanning block devices...");
        match Self::run_cmd("lsblk", &["--json", "-d", "-o", "NAME,SIZE,TYPE,TRAN"]).await {
            Ok(output) => match serde_json::from_str::<serde_json::Value>(&output) {
                Ok(json) => {
                    let devices: Vec<BlockDevice> = json["blockdevices"]
                        .as_array()
                        .unwrap_or(&Vec::new())
                        .iter()
                        .filter_map(|dev| {
                            let name = dev["name"].as_str()?.to_string();
                            let size = dev["size"].as_str().unwrap_or("?").to_string();
                            let dev_type = dev["type"].as_str().unwrap_or("");
                            let tran = dev["tran"].as_str().unwrap_or("").to_string();

                            // Filter out optical drives (rom) and loop devices
                            if dev_type == "rom" || dev_type == "loop" {
                                return None;
                            }

                            // Filter out the device the ISO is likely mounted from.
                            // USB-attached drives used as boot media typically have tran=usb
                            // and are often quite small. We also skip sr* devices.
                            if name.starts_with("sr") {
                                return None;
                            }

                            Some(BlockDevice { name, size, tran })
                        })
                        .collect();

                    // Check for HEARTH_TARGET_DISK env var to auto-select a disk.
                    if let Ok(target) = std::env::var("HEARTH_TARGET_DISK") {
                        if devices.iter().any(|d| d.name == target) {
                            self.log(format!(
                                "Auto-selecting disk from HEARTH_TARGET_DISK: /dev/{target}"
                            ));
                            self.target_disk = Some(target);
                            if self.disko_config.is_some() {
                                self.run_disko().await;
                            } else {
                                self.partition_disk().await;
                            }
                            return;
                        } else {
                            self.log(format!(
                                "Warning: HEARTH_TARGET_DISK={target} not found, falling back to selection"
                            ));
                        }
                    }

                    if devices.is_empty() {
                        self.state = ProvisionState::Error {
                            step: "disk discovery".into(),
                            message: "No suitable block devices found".into(),
                        };
                    } else if devices.len() == 1 {
                        self.log(format!(
                            "Single disk found: /dev/{} ({})",
                            devices[0].name, devices[0].size
                        ));
                        self.target_disk = Some(devices[0].name.clone());
                        self.state = ProvisionState::SelectDisk {
                            devices,
                            selected: 0,
                        };
                    } else {
                        self.log(format!("Found {} disks, please select one", devices.len()));
                        self.state = ProvisionState::SelectDisk {
                            devices,
                            selected: 0,
                        };
                    }
                }
                Err(e) => {
                    self.state = ProvisionState::Error {
                        step: "disk discovery".into(),
                        message: format!("Failed to parse lsblk output: {e}"),
                    };
                }
            },
            Err(e) => {
                self.state = ProvisionState::Error {
                    step: "disk discovery".into(),
                    message: e,
                };
            }
        }
    }

    /// Partition the selected disk with GPT: 512M EFI + rest as Linux root.
    async fn partition_disk(&mut self) {
        let disk = match &self.target_disk {
            Some(d) => d.clone(),
            None => {
                self.state = ProvisionState::Error {
                    step: "partitioning".into(),
                    message: "No disk selected".into(),
                };
                return;
            }
        };

        let dev = format!("/dev/{disk}");
        self.state = ProvisionState::Partitioning;
        self.log(format!("Wiping partition table on {dev}..."));

        // Zap existing partition table
        if let Err(e) = Self::run_cmd("sgdisk", &["--zap-all", &dev]).await {
            self.state = ProvisionState::Error {
                step: "partitioning".into(),
                message: format!("sgdisk --zap-all failed: {e}"),
            };
            return;
        }
        self.log("Partition table cleared.");

        // Create EFI partition (512MB)
        self.log("Creating EFI system partition (512MB)...");
        if let Err(e) = Self::run_cmd("sgdisk", &["-n", "1:0:+512M", "-t", "1:ef00", &dev]).await {
            self.state = ProvisionState::Error {
                step: "partitioning".into(),
                message: format!("Failed to create EFI partition: {e}"),
            };
            return;
        }
        self.log("EFI partition created.");

        // Create root partition (remaining space)
        self.log("Creating root partition...");
        if let Err(e) = Self::run_cmd("sgdisk", &["-n", "2:0:0", "-t", "2:8300", &dev]).await {
            self.state = ProvisionState::Error {
                step: "partitioning".into(),
                message: format!("Failed to create root partition: {e}"),
            };
            return;
        }
        self.log("Root partition created.");
        self.log("Partitioning complete.");

        // Move to formatting
        self.format_disk().await;
    }

    /// Format the partitions: FAT32 for EFI, ext4 for root.
    async fn format_disk(&mut self) {
        let disk = match &self.target_disk {
            Some(d) => d.clone(),
            None => {
                self.state = ProvisionState::Error {
                    step: "formatting".into(),
                    message: "No disk selected".into(),
                };
                return;
            }
        };

        self.state = ProvisionState::Formatting;
        let efi_part = Self::partition_path(&disk, 1);
        let root_part = Self::partition_path(&disk, 2);

        self.log(format!(
            "Formatting {efi_part} as FAT32 (EFI, label=boot)..."
        ));
        if let Err(e) = Self::run_cmd("mkfs.fat", &["-F", "32", "-n", "boot", &efi_part]).await {
            self.state = ProvisionState::Error {
                step: "formatting".into(),
                message: format!("Failed to format EFI partition: {e}"),
            };
            return;
        }
        self.log("EFI partition formatted.");

        self.log(format!("Formatting {root_part} as ext4 (label=nixos)..."));
        if let Err(e) = Self::run_cmd("mkfs.ext4", &["-F", "-L", "nixos", &root_part]).await {
            self.state = ProvisionState::Error {
                step: "formatting".into(),
                message: format!("Failed to format root partition: {e}"),
            };
            return;
        }
        self.log("Root partition formatted.");
        self.log("Formatting complete.");

        // Move to mounting
        self.mount_partitions().await;
    }

    /// Mount root at /mnt and EFI at /mnt/boot.
    async fn mount_partitions(&mut self) {
        let disk = match &self.target_disk {
            Some(d) => d.clone(),
            None => {
                self.state = ProvisionState::Error {
                    step: "mounting".into(),
                    message: "No disk selected".into(),
                };
                return;
            }
        };

        self.state = ProvisionState::Mounting;
        let efi_part = Self::partition_path(&disk, 1);
        let root_part = Self::partition_path(&disk, 2);

        // Ensure mount point exists
        if let Err(e) = Self::run_cmd("mkdir", &["-p", "/mnt"]).await {
            self.state = ProvisionState::Error {
                step: "mounting".into(),
                message: format!("Failed to create /mnt: {e}"),
            };
            return;
        }

        self.log(format!("Mounting {root_part} on /mnt..."));
        if let Err(e) = Self::run_cmd("mount", &[&root_part, "/mnt"]).await {
            self.state = ProvisionState::Error {
                step: "mounting".into(),
                message: format!("Failed to mount root: {e}"),
            };
            return;
        }
        self.log("Root partition mounted.");

        self.log("Creating /mnt/boot...");
        if let Err(e) = Self::run_cmd("mkdir", &["-p", "/mnt/boot"]).await {
            self.state = ProvisionState::Error {
                step: "mounting".into(),
                message: format!("Failed to create /mnt/boot: {e}"),
            };
            return;
        }

        self.log(format!("Mounting {efi_part} on /mnt/boot..."));
        if let Err(e) = Self::run_cmd("mount", &[&efi_part, "/mnt/boot"]).await {
            self.state = ProvisionState::Error {
                step: "mounting".into(),
                message: format!("Failed to mount EFI: {e}"),
            };
            return;
        }
        self.log("EFI partition mounted.");
        self.log("All partitions mounted.");

        // Move to installation
        self.install_system().await;
    }

    /// Run disko to partition, format, and mount the selected disk.
    ///
    /// The disko config is a Nix file bundled in the enrollment ISO at
    /// `/etc/hearth/disko-configs/{name}.nix`. It is a function `{ device }: ...`
    /// that disko evaluates to determine the partition layout.
    async fn run_disko(&mut self) {
        let disk = match &self.target_disk {
            Some(d) => d.clone(),
            None => {
                self.state = ProvisionState::Error {
                    step: "disko".into(),
                    message: "No disk selected".into(),
                };
                return;
            }
        };

        let config_name = self.disko_config.as_deref().unwrap_or("standard");
        let config_path = format!("/etc/hearth/disko-configs/{config_name}.nix");
        let dev = format!("/dev/{disk}");

        self.state = ProvisionState::RunningDisko;
        self.log(format!(
            "Running disko with config '{config_name}' on {dev}..."
        ));

        // disko [options] disk-config.nix — config file must be LAST.
        // --argstr (no hyphen) passes a string argument to the Nix expression.
        // --no-deps avoids fetching extra dependencies (use what's in the store).
        match Self::run_cmd(
            "disko",
            &[
                "--mode",
                "format,mount",
                "--argstr",
                "device",
                &dev,
                "--yes-wipe-all-disks",
                "--no-deps",
                &config_path,
            ],
        )
        .await
        {
            Ok(output) => {
                for line in output
                    .lines()
                    .rev()
                    .take(10)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                {
                    self.log(format!("  {line}"));
                }
                self.log("disko complete: disk partitioned, formatted, and mounted at /mnt");

                // Verify mount points before proceeding
                if let Err(e) = Self::run_cmd("mountpoint", &["-q", "/mnt"]).await {
                    self.state = ProvisionState::Error {
                        step: "disko".into(),
                        message: format!("/mnt is not a mount point after disko: {e}"),
                    };
                    return;
                }
                if let Err(e) = Self::run_cmd("mountpoint", &["-q", "/mnt/boot"]).await {
                    self.state = ProvisionState::Error {
                        step: "disko".into(),
                        message: format!("/mnt/boot is not a mount point after disko: {e}"),
                    };
                    return;
                }
                self.log("Mount points verified: /mnt and /mnt/boot OK");

                // Proceed to NixOS installation
                self.install_system().await;
            }
            Err(e) => {
                error!("disko failed: {e}");
                self.state = ProvisionState::Error {
                    step: "disko".into(),
                    message: format!("disko failed: {e}"),
                };
            }
        }
    }

    /// Write /etc/nix/netrc with bearer credentials for the cache server,
    /// enabling authenticated access during nixos-install.
    async fn write_netrc(&mut self, cache_url: &str, token: &str) -> Result<(), String> {
        // Extract the hostname (without port) from the cache URL for the netrc
        // machine field. Curl matches netrc entries by hostname only — including
        // the port would cause a silent auth failure.
        let host_port = cache_url
            .strip_prefix("http://")
            .or_else(|| cache_url.strip_prefix("https://"))
            .unwrap_or(cache_url)
            .split('/')
            .next()
            .unwrap_or(cache_url);
        let host = host_port.split(':').next().unwrap_or(host_port);

        let netrc_content = format!("machine {host}\nlogin bearer\npassword {token}\n");

        if let Err(e) = Self::run_cmd("mkdir", &["-p", "/etc/nix"]).await {
            return Err(format!("failed to create /etc/nix: {e}"));
        }

        tokio::fs::write("/etc/nix/netrc", netrc_content)
            .await
            .map_err(|e| format!("failed to write /etc/nix/netrc: {e}"))?;

        self.log(format!(
            "Wrote cache credentials for {host} to /etc/nix/netrc"
        ));
        Ok(())
    }

    /// Persist the enrollment-assigned machine identity to /mnt so the agent
    /// can read it on first boot. Called before nixos-install so that identity
    /// is preserved even if the install is interrupted or fails.
    async fn persist_machine_identity(&mut self) {
        if let Some(mid) = self.machine_id {
            let id_dir = "/mnt/var/lib/hearth";
            let id_path = format!("{id_dir}/machine-id");
            if let Err(e) = Self::run_cmd("mkdir", &["-p", id_dir]).await {
                warn!("failed to create {id_dir}: {e}");
                return;
            }
            if let Err(e) = tokio::fs::write(&id_path, mid.to_string()).await {
                warn!("failed to write machine-id to {id_path}: {e}");
            } else {
                self.log(format!("Machine identity written to {id_path}"));
            }

            // Write the machine auth token so the agent can
            // authenticate on first boot.
            if let Some(ref token) = self.machine_token {
                let token_path = format!("{id_dir}/machine-token");
                if let Err(e) = tokio::fs::write(&token_path, token).await {
                    warn!("failed to write machine-token to {token_path}: {e}");
                } else {
                    self.log("Machine auth token written");
                }
            }

            // Write the Headscale pre-auth key so the headscale-client
            // NixOS module can join the mesh on first boot.
            if let Some(ref key) = self.headscale_preauth_key {
                let key_path = format!("{id_dir}/headscale-key");
                if let Err(e) = tokio::fs::write(&key_path, key).await {
                    warn!("failed to write headscale-key to {key_path}: {e}");
                } else {
                    self.log("Headscale pre-auth key written");
                }
            }
        }
    }

    /// Run nixos-install with the target closure.
    async fn install_system(&mut self) {
        let closure = match &self.target_closure {
            Some(c) => c.clone(),
            None => {
                self.state = ProvisionState::Error {
                    step: "installation".into(),
                    message: "No target closure available".into(),
                };
                return;
            }
        };

        // Persist machine identity BEFORE nixos-install so it survives
        // interrupted installs and is available for test harnesses.
        self.persist_machine_identity().await;

        self.state = ProvisionState::Installing {
            progress: "Starting NixOS installation...".into(),
        };
        self.log(format!("Installing system closure: {closure}"));

        // Fetch a fresh cache token from the API before nixos-install.
        // The token from enrollment approval may have expired.
        if let Some(client) = &self.client {
            match client.get_cache_token().await {
                Ok(creds) => {
                    self.cache_url = Some(creds.cache_url);
                    self.cache_token = Some(creds.cache_token);
                    self.log("Obtained fresh cache credentials from API");
                }
                Err(e) => {
                    warn!(error = %e, "failed to get cache token, using existing credentials");
                }
            }
        }

        // If we have cache credentials, write netrc and configure substituters.
        let mut extra_args: Vec<String> = Vec::new();
        if let (Some(cache_url), Some(token)) = (self.cache_url.clone(), self.cache_token.clone()) {
            match self.write_netrc(&cache_url, &token).await {
                Ok(()) => {
                    let substituters = format!("{cache_url} https://cache.nixos.org");
                    extra_args.extend([
                        "--option".into(),
                        "substituters".into(),
                        substituters,
                        "--option".into(),
                        "netrc-file".into(),
                        "/etc/nix/netrc".into(),
                    ]);
                }
                Err(e) => {
                    warn!("failed to write netrc, proceeding without cache auth: {e}");
                }
            }
        }

        self.log("Running nixos-install (this may take a while)...");

        let mut args: Vec<&str> = vec![
            "--no-root-password",
            "--no-channel-copy",
            "--system",
            &closure,
        ];
        let extra_refs: Vec<&str> = extra_args.iter().map(|s| s.as_str()).collect();
        args.extend(extra_refs);

        match Self::run_cmd("nixos-install", &args).await {
            Ok(output) => {
                // Log the last few lines of output
                for line in output
                    .lines()
                    .rev()
                    .take(10)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                {
                    self.log(format!("  {line}"));
                }
                self.log("NixOS installation complete!");

                self.state = ProvisionState::Complete;
            }
            Err(e) => {
                error!("nixos-install failed: {e}");
                self.state = ProvisionState::Error {
                    step: "installation".into(),
                    message: format!("nixos-install failed: {e}"),
                };
            }
        }
    }

    /// Unmount and reboot.
    async fn reboot(&mut self) {
        self.state = ProvisionState::Rebooting;
        self.log("Unmounting filesystems...");
        if let Err(e) = Self::run_cmd("umount", &["-R", "/mnt"]).await {
            warn!("umount failed (continuing with reboot): {e}");
            self.log(format!("Warning: umount failed: {e}"));
        }
        self.log("Rebooting...");
        if let Err(e) = Self::run_cmd("reboot", &[]).await {
            self.state = ProvisionState::Error {
                step: "reboot".into(),
                message: format!("Reboot failed: {e}"),
            };
        }
    }

    /// Main tick driver — called each event loop iteration.
    /// Returns true if the provisioning is done and the app should exit.
    pub async fn tick(&mut self) -> bool {
        self.dots += 1;

        match &self.state {
            ProvisionState::WaitingForClosure => {
                // If we already have a closure, skip to disk discovery
                if self.target_closure.is_some() {
                    self.discover_disks().await;
                    return false;
                }

                // Poll the API every 3 seconds for the target closure.
                let should_poll = match self.last_poll {
                    Some(last) => last.elapsed().as_secs() >= 3,
                    None => true,
                };

                if should_poll {
                    self.last_poll = Some(Instant::now());

                    if let (Some(client), Some(machine_id)) = (&self.client, self.machine_id) {
                        match client.get_enrollment_status(machine_id).await {
                            Ok(resp) => {
                                if let Some(closure) = resp.target_closure {
                                    info!(%closure, "received target closure from API");
                                    self.target_closure = Some(closure);
                                } else if resp.build_error.is_some() {
                                    let err = resp.build_error.unwrap_or_default();
                                    let status = resp.build_status.unwrap_or_default();
                                    warn!(%status, %err, "build failed for this machine");
                                    self.state = ProvisionState::Error {
                                        step: "system build".into(),
                                        message: format!("Build failed: {err}"),
                                    };
                                } else {
                                    let status = resp.build_status.unwrap_or_else(|| "unknown".into());
                                    self.log(format!("Building system image... (status: {status})"));
                                }
                            }
                            Err(e) => {
                                warn!(error = %e, "failed to poll for closure");
                            }
                        }
                    } else {
                        self.log("Waiting for target closure...");
                    }
                }
            }
            // These states are driven by user input or are terminal
            ProvisionState::SelectDisk { .. }
            | ProvisionState::Complete
            | ProvisionState::Error { .. }
            | ProvisionState::Rebooting => {}
            // These are transient — they're driven by the async methods
            // and shouldn't appear during tick (they complete within a single
            // handle_key or tick call). But in case they do, don't do anything.
            ProvisionState::RunningDisko
            | ProvisionState::Partitioning
            | ProvisionState::Formatting
            | ProvisionState::Mounting
            | ProvisionState::Installing { .. } => {}
        }

        matches!(self.state, ProvisionState::Rebooting)
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let center = ui::centered_rect(75, 80, area);
        let block = ui::hearth_block(" Provisioning ");

        let inner = block.inner(center);
        frame.render_widget(block, center);

        match &self.state {
            ProvisionState::WaitingForClosure => {
                self.render_waiting_for_closure(frame, inner);
            }
            ProvisionState::SelectDisk { devices, selected } => {
                self.render_select_disk(frame, inner, devices, *selected);
            }
            ProvisionState::RunningDisko => {
                self.render_progress(frame, inner, "Partitioning (disko)", Color::Yellow);
            }
            ProvisionState::Partitioning => {
                self.render_progress(frame, inner, "Partitioning", Color::Yellow);
            }
            ProvisionState::Formatting => {
                self.render_progress(frame, inner, "Formatting", Color::Yellow);
            }
            ProvisionState::Mounting => {
                self.render_progress(frame, inner, "Mounting", Color::Yellow);
            }
            ProvisionState::Installing { progress } => {
                self.render_installing(frame, inner, progress);
            }
            ProvisionState::Complete => {
                self.render_complete(frame, inner);
            }
            ProvisionState::Rebooting => {
                self.render_rebooting(frame, inner);
            }
            ProvisionState::Error { step, message } => {
                self.render_error(frame, inner, step, message);
            }
        }
    }

    fn render_waiting_for_closure(&self, frame: &mut Frame, area: Rect) {
        let dots_str = ".".repeat((self.dots % 4) + 1);
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  Waiting for system image{dots_str}"),
                Style::default().fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!(
                    "  Machine ID: {}",
                    self.machine_id
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "unknown".into())
                ),
                Style::default().fg(ui::MUTED),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  The control plane will assign a system closure to this",
                Style::default().fg(ui::MUTED),
            )),
            Line::from(Span::styled(
                "  device. This happens after an admin approves enrollment",
                Style::default().fg(ui::MUTED),
            )),
            Line::from(Span::styled(
                "  and sets a target configuration.",
                Style::default().fg(ui::MUTED),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Polling automatically every 3 seconds...",
                Style::default().fg(ui::MUTED),
            )),
        ];
        frame.render_widget(Paragraph::new(lines), area);
    }

    fn render_select_disk(
        &self,
        frame: &mut Frame,
        area: Rect,
        devices: &[BlockDevice],
        selected: usize,
    ) {
        // Split the area: header at top, device list in middle, footer at bottom
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(4),
                Constraint::Length(3),
            ])
            .split(area);

        // Header
        let header = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Select the target disk for installation:",
                Style::default().fg(Color::White),
            )),
        ];
        frame.render_widget(Paragraph::new(header), chunks[0]);

        // Device list
        let items: Vec<ListItem> = devices
            .iter()
            .enumerate()
            .map(|(i, dev)| {
                let marker = if i == selected { "> " } else { "  " };
                let transport = if dev.tran.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", dev.tran)
                };
                let text = format!("{marker}/dev/{:<12} {:>10}{transport}", dev.name, dev.size);
                let style = if i == selected {
                    Style::default().fg(ui::EMBER).bold()
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(Line::from(Span::styled(text, style)))
            })
            .collect();

        let list = List::new(items);
        frame.render_widget(list, chunks[1]);

        // Footer
        let footer = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Up/Down to select  |  Enter to confirm  |  ALL DATA WILL BE ERASED",
                Style::default().fg(ui::MUTED),
            )),
        ];
        frame.render_widget(Paragraph::new(footer), chunks[2]);
    }

    fn render_progress(&self, frame: &mut Frame, area: Rect, step_name: &str, color: Color) {
        let dots_str = ".".repeat((self.dots % 4) + 1);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(4), Constraint::Min(1)])
            .split(area);

        // Step header
        let header = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  {step_name}{dots_str}"),
                Style::default().fg(color),
            )),
            Line::from(""),
        ];
        frame.render_widget(Paragraph::new(header), chunks[0]);

        // Log lines (show last N that fit)
        self.render_log_lines(frame, chunks[1]);
    }

    fn render_installing(&self, frame: &mut Frame, area: Rect, progress: &str) {
        let dots_str = ".".repeat((self.dots % 4) + 1);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(5), Constraint::Min(1)])
            .split(area);

        let header = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  Installing NixOS{dots_str}"),
                Style::default().fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!("  {progress}"),
                Style::default().fg(ui::MUTED),
            )),
        ];
        frame.render_widget(Paragraph::new(header), chunks[0]);

        self.render_log_lines(frame, chunks[1]);
    }

    fn render_complete(&self, frame: &mut Frame, area: Rect) {
        let lines = vec![
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "  Installation complete!",
                Style::default().fg(Color::Green).bold(),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  NixOS has been successfully installed to disk.",
                Style::default().fg(Color::White),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  The system is ready. Remove the installation media",
                Style::default().fg(ui::MUTED),
            )),
            Line::from(Span::styled(
                "  and reboot into the new system.",
                Style::default().fg(ui::MUTED),
            )),
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "  Press Enter to reboot",
                Style::default().fg(ui::EMBER).bold(),
            )),
        ];
        frame.render_widget(Paragraph::new(lines), area);
    }

    fn render_rebooting(&self, frame: &mut Frame, area: Rect) {
        let dots_str = ".".repeat((self.dots % 4) + 1);
        let lines = vec![
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                format!("  Rebooting{dots_str}"),
                Style::default().fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Unmounting filesystems and restarting...",
                Style::default().fg(ui::MUTED),
            )),
        ];
        frame.render_widget(Paragraph::new(lines), area);
    }

    fn render_error(&self, frame: &mut Frame, area: Rect, step: &str, message: &str) {
        let header = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  Error during: {step}"),
                Style::default().fg(Color::Red).bold(),
            )),
            Line::from(""),
        ];
        let max_width = area.width.saturating_sub(4) as usize;
        let err_lines = ui::textwrap_lines(message, max_width, Color::Red);
        let footer = vec![
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "  Press Enter to retry  |  q to quit",
                Style::default().fg(ui::MUTED),
            )),
        ];
        let all: Vec<Line> = header
            .into_iter()
            .chain(err_lines)
            .chain(footer)
            .collect();
        frame.render_widget(Paragraph::new(all), area);
    }

    fn render_log_lines(&self, frame: &mut Frame, area: Rect) {
        let max_lines = area.height as usize;
        let start = self.log_lines.len().saturating_sub(max_lines);
        let visible: Vec<Line> = self.log_lines[start..]
            .iter()
            .map(|l| {
                Line::from(Span::styled(
                    format!("  {l}"),
                    Style::default().fg(ui::MUTED),
                ))
            })
            .collect();
        frame.render_widget(Paragraph::new(visible), area);
    }

    /// Handle key input. Returns true if the app should transition to Done/exit.
    pub async fn handle_key(&mut self, key: KeyEvent) -> bool {
        match &self.state {
            ProvisionState::SelectDisk { devices, selected } => {
                let selected = *selected;
                let len = devices.len();
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if let ProvisionState::SelectDisk {
                            selected: ref mut s,
                            ..
                        } = self.state
                        {
                            *s = selected.saturating_sub(1);
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if let ProvisionState::SelectDisk {
                            selected: ref mut s,
                            ..
                        } = self.state
                        {
                            *s = (selected + 1).min(len.saturating_sub(1));
                        }
                    }
                    KeyCode::Enter => {
                        // Confirm disk selection and start partitioning
                        if let ProvisionState::SelectDisk {
                            ref devices,
                            selected,
                        } = self.state
                        {
                            let disk_name = devices[selected].name.clone();
                            self.log(format!("Selected disk: /dev/{disk_name}"));
                            self.target_disk = Some(disk_name);
                        }
                        // Use disko if a config was provided, otherwise fall back
                        // to manual sgdisk/mkfs partitioning.
                        if self.disko_config.is_some() {
                            self.run_disko().await;
                        } else {
                            self.partition_disk().await;
                        }
                    }
                    _ => {}
                }
            }
            ProvisionState::Complete => {
                if matches!(key.code, KeyCode::Enter) {
                    self.reboot().await;
                }
            }
            ProvisionState::Error { step, .. } => {
                if key.code == KeyCode::Enter {
                    let step = step.clone();
                    self.log(format!("Retrying from: {step}"));
                    // Retry from the failed step
                    match step.as_str() {
                        "disk discovery" => {
                            self.state = ProvisionState::WaitingForClosure;
                            // Will re-enter disk discovery on next tick if closure exists
                        }
                        "disko" => {
                            self.run_disko().await;
                        }
                        "partitioning" => {
                            self.partition_disk().await;
                        }
                        "formatting" => {
                            self.format_disk().await;
                        }
                        "mounting" => {
                            self.mount_partitions().await;
                        }
                        "installation" => {
                            self.install_system().await;
                        }
                        "reboot" => {
                            self.reboot().await;
                        }
                        _ => {
                            // Fall back to waiting for closure
                            self.state = ProvisionState::WaitingForClosure;
                        }
                    }
                }
            }
            ProvisionState::Rebooting => {
                // After reboot is triggered, signal that we're done
                return true;
            }
            // No input handling for transient states
            _ => {}
        }
        false
    }

    /// Whether the user is allowed to quit from this screen.
    pub fn can_quit(&self) -> bool {
        matches!(
            self.state,
            ProvisionState::WaitingForClosure | ProvisionState::Error { .. }
        )
    }
}
