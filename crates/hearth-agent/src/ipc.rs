//! Unix-socket IPC server for communication with the hearth-greeter.
//!
//! The protocol is newline-delimited JSON: each message is a single JSON
//! object terminated by `\n`. The greeter sends [`AgentRequest`]s and the
//! agent responds with [`AgentEvent`]s.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use hearth_common::api_client::HearthApiClient;
use hearth_common::api_types::{DesktopPreferences, SyncDesktopPrefsRequest, UserEnvStatus};
use hearth_common::config::AgentConfig;
use hearth_common::ipc::{AgentEvent, AgentRequest};

/// Tracks the preparation status for each user.
#[derive(Debug, Clone)]
enum PrepareStatus {
    Preparing,
    Ready,
    Error(String),
}

/// Shared state across IPC connections.
struct IpcState<C> {
    prepare_status: HashMap<String, PrepareStatus>,
    client: Arc<C>,
    config: Arc<AgentConfig>,
    machine_id: Uuid,
}

/// Resolve a user's role from their group memberships using the configured mapping.
///
/// Returns the first matching role, or the default role if no mapping matches
/// (or `"default"` if no role mapping is configured at all).
fn resolve_role(groups: &[String], config: &AgentConfig) -> String {
    if let Some(mapping) = &config.role_mapping {
        for entry in &mapping.mappings {
            if groups.contains(&entry.group) {
                return entry.role.clone();
            }
        }
        warn!(
            ?groups,
            default_role = %mapping.default_role,
            "no role mapping matched user's groups, using default"
        );
        mapping.default_role.clone()
    } else {
        warn!("no role_mapping configured, using \"default\"");
        "default".into()
    }
}

/// Run the IPC server on the given Unix socket path.
///
/// Listens for incoming connections, spawns a task per connection, and
/// shuts down cleanly when the cancellation token fires.
/// Try to receive a socket-activated listener from systemd (LISTEN_FDS protocol).
///
/// The caller must have already called `consume_listen_fds()` before starting
/// the async runtime to safely unset the environment variables.
///
/// Returns `Some(UnixListener)` if fd 3 is available and LISTEN_FDS=1.
fn try_socket_activation() -> Option<UnixListener> {
    use std::os::unix::io::FromRawFd;

    // Check the cached value from consume_listen_fds().
    let listen_fds = LISTEN_FDS_COUNT.load(std::sync::atomic::Ordering::Relaxed);
    if listen_fds < 1 {
        return None;
    }

    // systemd passes fds starting at 3 (SD_LISTEN_FDS_START).
    let fd = 3;

    // SAFETY: fd 3 is passed by systemd via socket activation and is a
    // valid, open Unix stream socket. We take ownership of it.
    let std_listener = unsafe { std::os::unix::net::UnixListener::from_raw_fd(fd) };
    std_listener.set_nonblocking(true).ok()?;
    let listener = UnixListener::from_std(std_listener).ok()?;

    info!("using socket-activated listener from systemd (fd 3)");
    Some(listener)
}

/// Cached LISTEN_FDS count, consumed from env before tokio starts.
static LISTEN_FDS_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Consume LISTEN_FDS/LISTEN_PID from the environment. Must be called from
/// main() before spawning any threads, as `env::remove_var` is unsafe in
/// multi-threaded contexts.
pub fn consume_listen_fds() {
    let pid_matches = std::env::var("LISTEN_PID")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .is_some_and(|pid| pid == std::process::id());

    let fds = std::env::var("LISTEN_FDS")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    if pid_matches && fds > 0 {
        LISTEN_FDS_COUNT.store(fds, std::sync::atomic::Ordering::Relaxed);

        // SAFETY: Called from main() before the tokio runtime starts, so no
        // other threads are reading these env vars concurrently.
        unsafe {
            std::env::remove_var("LISTEN_FDS");
            std::env::remove_var("LISTEN_PID");
        }
    }
}

pub async fn run_ipc_server<C: HearthApiClient + 'static>(
    socket_path: &str,
    client: Arc<C>,
    config: Arc<AgentConfig>,
    machine_id: Uuid,
    shutdown: CancellationToken,
    ready_tx: Option<tokio::sync::oneshot::Sender<()>>,
) {
    // Prefer a socket-activated listener from systemd. This is the correct
    // approach for socket permissions: systemd creates the socket with the
    // right ownership/group (configured in the .socket unit), avoiding the
    // need for supplementary groups or ACLs.
    let listener = if let Some(l) = try_socket_activation() {
        l
    } else {
        // Fallback: bind the socket ourselves (development / non-systemd use).
        let path = Path::new(socket_path);

        if path.exists()
            && let Err(e) = std::fs::remove_file(path)
        {
            error!(path = %socket_path, error = %e, "failed to remove stale socket file");
            return;
        }

        if let Some(parent) = path.parent()
            && !parent.exists()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            error!(
                path = %parent.display(),
                error = %e,
                "failed to create socket parent directory"
            );
            return;
        }

        match UnixListener::bind(socket_path) {
            Ok(l) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let perms = std::fs::Permissions::from_mode(0o660);
                    if let Err(e) = std::fs::set_permissions(socket_path, perms) {
                        warn!(
                            path = %socket_path,
                            error = %e,
                            "failed to set socket permissions"
                        );
                    }
                }
                l
            }
            Err(e) => {
                error!(path = %socket_path, error = %e, "failed to bind Unix socket");
                return;
            }
        }
    };

    info!(path = %socket_path, "IPC server listening");

    // Signal that the socket is ready for connections.
    if let Some(tx) = ready_tx {
        let _ = tx.send(());
    }

    let state = Arc::new(Mutex::new(IpcState {
        prepare_status: HashMap::new(),
        client,
        config,
        machine_id,
    }));

    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, _addr)) => {
                        debug!("accepted IPC connection");
                        let state = Arc::clone(&state);
                        let shutdown = shutdown.clone();
                        tokio::spawn(async move {
                            handle_connection(stream, state, shutdown).await;
                        });
                    }
                    Err(e) => {
                        warn!(error = %e, "failed to accept IPC connection");
                    }
                }
            }
            () = shutdown.cancelled() => {
                info!("IPC server shutting down");
                break;
            }
        }
    }

    // Clean up the socket file on shutdown.
    if let Err(e) = std::fs::remove_file(socket_path) {
        debug!(error = %e, "failed to remove socket file during shutdown (may already be gone)");
    }
}

/// Handle a single IPC connection.
///
/// Reads newline-delimited JSON requests, processes each one, and writes
/// back newline-delimited JSON responses. Background tasks (e.g. environment
/// preparation) send events back through an mpsc channel that is multiplexed
/// with the incoming request stream.
async fn handle_connection<C: HearthApiClient + 'static>(
    stream: tokio::net::UnixStream,
    state: Arc<Mutex<IpcState<C>>>,
    shutdown: CancellationToken,
) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    // Channel for background tasks to send events back to this connection.
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(32);

    loop {
        tokio::select! {
            // Forward events from background tasks to the client.
            Some(event) = event_rx.recv() => {
                if let Err(e) = send_event(&mut writer, &event).await {
                    warn!(error = %e, "failed to forward background event to client");
                    return;
                }
            }

            // Read incoming requests from the client.
            result = lines.next_line() => {
                let line = match result {
                    Ok(Some(line)) => line,
                    Ok(None) => {
                        debug!("IPC client disconnected");
                        return;
                    }
                    Err(e) => {
                        warn!(error = %e, "error reading from IPC client");
                        return;
                    }
                };

                let request: AgentRequest = match serde_json::from_str(&line) {
                    Ok(req) => req,
                    Err(e) => {
                        warn!(error = %e, raw = %line, "invalid IPC request");
                        let event = AgentEvent::Error {
                            username: String::new(),
                            message: format!("invalid request: {e}"),
                        };
                        if let Err(e) = send_event(&mut writer, &event).await {
                            warn!(error = %e, "failed to send error response");
                        }
                        continue;
                    }
                };

                debug!(?request, "handling IPC request");

                match request {
                    AgentRequest::Ping => {
                        if let Err(e) = send_event(&mut writer, &AgentEvent::Pong).await {
                            warn!(error = %e, "failed to send Pong");
                            return;
                        }
                    }

                    AgentRequest::PrepareUserEnv { username, groups } => {
                        handle_prepare_user_env(
                            &username,
                            groups,
                            &state,
                            &mut writer,
                            &event_tx,
                            &shutdown,
                        )
                        .await;
                    }

                    AgentRequest::GetPrepareStatus { username } => {
                        let st = state.lock().await;
                        let event = match st.prepare_status.get(&username) {
                            Some(PrepareStatus::Preparing) => AgentEvent::Preparing {
                                username,
                                message: "still preparing".into(),
                            },
                            Some(PrepareStatus::Ready) => AgentEvent::Ready { username },
                            Some(PrepareStatus::Error(msg)) => AgentEvent::Error {
                                username,
                                message: msg.clone(),
                            },
                            None => AgentEvent::Error {
                                username,
                                message: "no preparation has been requested for this user".into(),
                            },
                        };
                        if let Err(e) = send_event(&mut writer, &event).await {
                            warn!(error = %e, "failed to send prepare status");
                            return;
                        }
                    }
                }
            }

            () = shutdown.cancelled() => {
                debug!("IPC connection closing due to shutdown");
                return;
            }
        }
    }
}

/// Handle a `PrepareUserEnv` request: send an immediate acknowledgement, then
/// spawn a background task that performs the actual activation and streams
/// progress events back through `event_tx`.
async fn handle_prepare_user_env<C: HearthApiClient + 'static>(
    username: &str,
    groups: Vec<String>,
    state: &Arc<Mutex<IpcState<C>>>,
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    event_tx: &mpsc::Sender<AgentEvent>,
    shutdown: &CancellationToken,
) {
    debug!(%username, ?groups, "preparing user environment");

    // Record that preparation has started.
    {
        let mut st = state.lock().await;
        st.prepare_status
            .insert(username.to_string(), PrepareStatus::Preparing);
    }

    // Send immediate "Preparing" acknowledgement.
    let preparing = AgentEvent::Preparing {
        username: username.to_string(),
        message: "preparing user environment".into(),
    };
    if let Err(e) = send_event(writer, &preparing).await {
        warn!(error = %e, "failed to send Preparing event");
        return;
    }

    // Spawn a background task to build/activate the user environment.
    let state_bg = Arc::clone(state);
    let bg_shutdown = shutdown.clone();
    let event_tx = event_tx.clone();
    let user = username.to_string();
    tokio::spawn(async move {
        // Single lock acquisition to extract everything we need.
        let (role, client, machine_id) = {
            let st = state_bg.lock().await;
            let role = resolve_role(&groups, &st.config);
            let client = Arc::clone(&st.client);
            let machine_id = st.machine_id;
            (role, client, machine_id)
        };
        info!(%user, %role, "resolved role for user");

        // Send progress: preparing.
        let _ = event_tx
            .send(AgentEvent::Progress {
                username: user.clone(),
                percent: 10,
                message: format!("preparing {role} environment"),
            })
            .await;

        // Resolve the user via passwd and ensure their home directory exists.
        // The resolved passwd name may be the full SPN (e.g., testuser@kanidm.hearth.local)
        // which we use for API calls so buildUserEnv gets the correct homeDirectory.
        let resolved = match resolve_and_ensure_home(&user) {
            Ok(r) => {
                info!(%user, passwd_name = %r.passwd_name, home = %r.home, "resolved user identity");
                Some(r)
            }
            Err(e) => {
                warn!(error = %e, "failed to resolve user (continuing with greeter username)");
                None
            }
        };
        let api_user = resolved
            .as_ref()
            .map(|r| r.passwd_name.as_str())
            .unwrap_or(&user);

        // Report "building" status to control plane.
        if let Err(e) = client
            .report_user_env(machine_id, api_user, &role, UserEnvStatus::Building)
            .await
        {
            warn!(error = %e, "failed to report building status");
        }

        // Try to use a pre-built per-user closure from the control plane.
        // If available, pull it from the cache and activate it.
        // If not, poll until one is built.
        let activation_result = {
            // Step 1: Check for pre-built closure.
            let prebuilt = match client.get_user_env_closure(api_user, Some(&role)).await {
                Ok(resp) => resp,
                Err(e) => {
                    error!(error = %e, "failed to reach control plane for per-user closure");

                    let _ = client
                        .report_user_env(machine_id, api_user, &role, UserEnvStatus::Failed)
                        .await;

                    let msg = format!(
                        "Cannot reach the control plane to prepare your environment. \
                         Check network connectivity or contact IT support. ({})",
                        e
                    );
                    let mut st = state_bg.lock().await;
                    st.prepare_status
                        .insert(user.clone(), PrepareStatus::Error(msg.clone()));
                    let _ = event_tx
                        .send(AgentEvent::Error {
                            username: user,
                            message: msg,
                        })
                        .await;
                    return;
                }
            };

            if let Some(closure) = &prebuilt.closure {
                // Validate the closure path before using it.
                if !hearth_common::nix_store::is_valid_store_path(closure) {
                    error!(%user, %closure, "invalid closure path from control plane");
                    return;
                }

                // Pre-built per-user closure available — pull and activate.
                info!(%user, %closure, "activating pre-built per-user closure");

                let _ = event_tx
                    .send(AgentEvent::Progress {
                        username: user.clone(),
                        percent: 30,
                        message: "pulling pre-built environment from cache".into(),
                    })
                    .await;

                // Realise the closure using the system's configured substituters.
                // This pulls Hearth-specific paths from the Attic binary cache
                // and standard nixpkgs paths from cache.nixos.org, streaming
                // per-path progress to the greeter.
                if let Err(msg) =
                    realise_closure_with_progress(closure, &event_tx, &user, &bg_shutdown).await
                {
                    error!(%user, %closure, %msg, "nix-store --realise failed");
                    let _ = client.report_closure_failure(api_user, closure, &msg).await;
                    Err(msg)
                } else {
                    info!(%user, %closure, "closure realised from substituters");
                    let _ = event_tx
                        .send(AgentEvent::Progress {
                            username: user.clone(),
                            percent: 60,
                            message: "activating per-user environment".into(),
                        })
                        .await;

                    // Activate the home-manager generation.
                    let activate_path = format!("{closure}/activate");
                    run_as_user(&user, &activate_path, &[], &bg_shutdown).await
                }
            } else {
                // No pre-built closure yet — the build may be in progress.
                // Poll the API for the closure to become available.
                info!(%user, %role, "no pre-built closure yet, waiting for build");

                const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);
                const MAX_WAIT: std::time::Duration = std::time::Duration::from_secs(300);
                let start = std::time::Instant::now();
                let mut closure_path: Option<String> = None;

                while start.elapsed() < MAX_WAIT {
                    if bg_shutdown.is_cancelled() {
                        break;
                    }

                    let elapsed_pct =
                        (start.elapsed().as_secs() * 50 / MAX_WAIT.as_secs()).min(50) as u8;
                    let _ = event_tx
                        .send(AgentEvent::Progress {
                            username: user.clone(),
                            percent: 10 + elapsed_pct,
                            message: "waiting for environment build to complete...".into(),
                        })
                        .await;

                    tokio::time::sleep(POLL_INTERVAL).await;

                    match client.get_user_env_closure(api_user, Some(&role)).await {
                        Ok(resp) if resp.closure.is_some() => {
                            closure_path = resp.closure;
                            break;
                        }
                        Ok(resp) => {
                            use hearth_common::api_types::UserEnvBuildStatus;
                            let status_msg = match resp.build_status {
                                Some(UserEnvBuildStatus::Pending) => "environment build queued...",
                                Some(UserEnvBuildStatus::Building) => {
                                    "environment is being built..."
                                }
                                Some(UserEnvBuildStatus::Failed) => {
                                    "environment build failed, retrying..."
                                }
                                _ => "waiting for environment build...",
                            };
                            let _ = event_tx
                                .send(AgentEvent::Progress {
                                    username: user.clone(),
                                    percent: 10 + elapsed_pct,
                                    message: status_msg.into(),
                                })
                                .await;
                            debug!(%user, ?resp.build_status, "closure still not ready, polling...");
                        }
                        Err(e) => {
                            debug!(%user, error = %e, "failed to poll for closure");
                        }
                    }
                }

                if let Some(closure) = &closure_path {
                    if !hearth_common::nix_store::is_valid_store_path(closure) {
                        Err(format!("invalid closure path: {closure}"))
                    } else {
                        info!(%user, %closure, "closure became available, realising");
                        let _ = event_tx
                            .send(AgentEvent::Progress {
                                username: user.clone(),
                                percent: 60,
                                message: "pulling environment from cache".into(),
                            })
                            .await;

                        match realise_closure_with_progress(closure, &event_tx, &user, &bg_shutdown)
                            .await
                        {
                            Err(msg) => {
                                let _ =
                                    client.report_closure_failure(api_user, closure, &msg).await;
                                Err(msg)
                            }
                            Ok(()) => {
                                let _ = event_tx
                                    .send(AgentEvent::Progress {
                                        username: user.clone(),
                                        percent: 80,
                                        message: "activating per-user environment".into(),
                                    })
                                    .await;
                                let activate_path = format!("{closure}/activate");
                                run_as_user(&user, &activate_path, &[], &bg_shutdown).await
                            }
                        }
                    }
                } else {
                    let _ = client
                        .report_user_env(machine_id, api_user, &role, UserEnvStatus::Failed)
                        .await;

                    error!(
                        %user,
                        %role,
                        "environment build timed out after {} seconds",
                        MAX_WAIT.as_secs()
                    );

                    Err(format!(
                        "Your environment could not be built within {} minutes. \
                         The build server may be overloaded or unreachable. \
                         Please contact IT support. (user: {}, role: {})",
                        MAX_WAIT.as_secs() / 60,
                        user,
                        role
                    ).into())
                }
            }
        };

        match activation_result {
            Ok(()) => {
                // Report active status + record login.
                if let Err(e) = client
                    .report_user_env(machine_id, api_user, &role, UserEnvStatus::Active)
                    .await
                {
                    warn!(error = %e, "failed to report active status");
                }
                if let Err(e) = client.report_user_login(machine_id, api_user).await {
                    warn!(error = %e, "failed to report user login");
                }

                // Sync observed desktop preferences back to the control plane.
                // Run in a spawned task to avoid delaying the Ready signal.
                let sync_client = client.clone();
                let sync_user = api_user.to_string();
                tokio::spawn(async move {
                    sync_user_desktop_prefs(&*sync_client, machine_id, &sync_user).await;
                });

                let mut st = state_bg.lock().await;
                st.prepare_status.insert(user.clone(), PrepareStatus::Ready);

                let _ = event_tx.send(AgentEvent::Ready { username: user }).await;
            }
            Err(msg) => {
                error!(%user, error = %msg, "user environment activation failed");

                // Enrich the error with disk usage info when space is the issue.
                let error_msg = if msg.contains("No space left on device") {
                    let disk_info = get_disk_usage_summary().await;
                    format!("No space left on device.\n\nDisk usage:\n{disk_info}")
                } else {
                    msg
                };

                if let Err(e) = client
                    .report_user_env(machine_id, api_user, &role, UserEnvStatus::Failed)
                    .await
                {
                    warn!(error = %e, "failed to report failed status");
                }

                let mut st = state_bg.lock().await;
                st.prepare_status
                    .insert(user.clone(), PrepareStatus::Error(error_msg.clone()));

                let _ = event_tx
                    .send(AgentEvent::Error {
                        username: user,
                        message: error_msg,
                    })
                    .await;
            }
        }
    });
}

/// Ensure a user's home directory exists before environment activation.
///
/// The agent runs as root and prepares the user environment *before* the PAM
/// session is opened (which is when `pam_mkhomedir` would normally create
/// the home directory). We must create it ourselves so that `runuser`-based
/// activation can write to `$HOME`.
/// Resolved user identity from the passwd database.
struct ResolvedUser {
    /// The canonical username from passwd (e.g., the Kanidm SPN).
    passwd_name: String,
    /// The user's home directory path.
    home: String,
}

/// Resolve a user via getent and ensure their home directory exists.
///
/// Returns the canonical passwd username (which may differ from the input
/// when Kanidm resolves a short name to an SPN) and the home directory path.
fn resolve_and_ensure_home(username: &str) -> Result<ResolvedUser, String> {
    let output = std::process::Command::new("getent")
        .args(["passwd", username])
        .output()
        .map_err(|e| format!("getent passwd failed: {e}"))?;

    if !output.status.success() {
        return Err(format!("user {username} not found in passwd database"));
    }

    let entry = String::from_utf8_lossy(&output.stdout);
    let fields: Vec<&str> = entry.trim().split(':').collect();
    if fields.len() < 7 {
        return Err(format!("invalid passwd entry for {username}"));
    }

    let passwd_name = fields[0].to_string();
    let uid: u32 = fields[2]
        .parse()
        .map_err(|_| format!("invalid uid in passwd entry for {username}"))?;
    let gid: u32 = fields[3]
        .parse()
        .map_err(|_| format!("invalid gid in passwd entry for {username}"))?;
    let home = fields[5].to_string();

    let home_path = std::path::Path::new(&home);
    if !home_path.exists() {
        info!(%username, %home, "creating home directory");
        std::fs::create_dir_all(home_path)
            .map_err(|e| format!("failed to create home directory {home}: {e}"))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::chown;
            chown(home_path, Some(uid), Some(gid))
                .map_err(|e| format!("failed to chown {home} to {uid}:{gid}: {e}"))?;
        }
    }

    Ok(ResolvedUser { passwd_name, home })
}

/// Realise a Nix store closure, streaming download progress to the greeter.
///
/// `nix-store --realise` outputs `copying path '...' from '...'` lines on
/// stderr. We parse these and forward human-friendly messages (e.g.,
/// "fetching firefox-147.0.3") as `AgentEvent::Progress` events.
async fn realise_closure_with_progress(
    closure: &str,
    event_tx: &mpsc::Sender<AgentEvent>,
    username: &str,
    shutdown: &CancellationToken,
) -> Result<(), String> {
    use tokio::io::AsyncBufReadExt;

    let mut child = tokio::process::Command::new("nix-store")
        .args(["--realise", closure])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("nix-store --realise failed to start: {e}"))?;

    let stderr = child.stderr.take();
    let progress_user = username.to_string();
    let progress_tx = event_tx.clone();

    let stderr_task = tokio::spawn(async move {
        let mut collected = String::new();
        let Some(stderr) = stderr else {
            return collected;
        };
        let reader = tokio::io::BufReader::new(stderr);
        let mut lines = reader.lines();
        let mut fetched: usize = 0;
        let mut total_paths: usize = 0;
        let mut total_download_mib: Option<String> = None;
        // Track in-flight downloads (started but not yet completed)
        let mut in_flight: Vec<String> = Vec::new();

        while let Ok(Some(line)) = lines.next_line().await {
            collected.push_str(&line);
            collected.push('\n');

            // "these 157 paths will be fetched (312.50 MiB download, 1024.00 MiB unpacked):"
            if total_paths == 0 {
                if let Some(n) = parse_paths_to_fetch_count(&line) {
                    total_paths = n;
                    // Extract download size if present
                    if let Some(start) = line.find('(') {
                        if let Some(end) = line.find(" download") {
                            total_download_mib = Some(line[start + 1..end].to_string());
                        }
                    }
                    let size_info = total_download_mib
                        .as_deref()
                        .map(|s| format!(" — {s}"))
                        .unwrap_or_default();
                    let _ = progress_tx
                        .send(AgentEvent::Progress {
                            username: progress_user.clone(),
                            percent: 20,
                            message: format!("downloading {total_paths} packages{size_info}"),
                        })
                        .await;
                    continue;
                }
            }

            if let Some(name) = parse_fetch_name(&line) {
                fetched += 1;
                // Remove from in-flight if present
                in_flight.retain(|n| n != &name);

                let pct = if total_paths > 0 {
                    (20 + (fetched * 40 / total_paths).min(40)) as u8
                } else {
                    40
                };

                // Send package name on first line, count on second line.
                // The greeter shows the name as status text and the count
                // inside the GTK ProgressBar widget.
                let count_text = if total_paths > 0 {
                    format!("{fetched}/{total_paths}")
                } else {
                    format!("{fetched} fetched")
                };

                let _ = progress_tx
                    .send(AgentEvent::Progress {
                        username: progress_user.clone(),
                        percent: pct,
                        message: format!("{name}\n{count_text}"),
                    })
                    .await;
            } else if let Some(msg) = parse_nix_progress(&line) {
                // Building derivations, etc.
                let _ = progress_tx
                    .send(AgentEvent::Progress {
                        username: progress_user.clone(),
                        percent: 55,
                        message: msg,
                    })
                    .await;
            }
        }
        collected
    });

    tokio::select! {
        status = child.wait() => {
            let stderr_output = stderr_task.await.unwrap_or_default();
            match status {
                Ok(s) if s.success() => Ok(()),
                Ok(_) => Err(format!("nix-store --realise failed: {stderr_output}")),
                Err(e) => Err(format!("nix-store --realise failed: {e}")),
            }
        }
        () = shutdown.cancelled() => {
            let _ = child.kill().await;
            Err("shutdown during closure realise".into())
        }
    }
}

/// Run a command as a specific user via `runuser`, with shutdown support.
///
/// Returns `Ok(())` on success, `Err(message)` on failure.
async fn run_as_user(
    username: &str,
    command: &str,
    args: &[&str],
    shutdown: &CancellationToken,
) -> Result<(), String> {
    run_as_user_with_progress(username, command, args, shutdown, None).await
}

/// Run a command as a specific user via `runuser`, streaming stderr lines
/// as progress events to the greeter when `progress_tx` is provided.
async fn run_as_user_with_progress(
    username: &str,
    command: &str,
    args: &[&str],
    shutdown: &CancellationToken,
    progress_tx: Option<(&mpsc::Sender<AgentEvent>, &str)>,
) -> Result<(), String> {
    let mut cmd_args = vec!["-u", username, "--", command];
    cmd_args.extend(args);

    let mut child = tokio::process::Command::new("runuser")
        .args(&cmd_args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to run {command}: {e}"))?;

    // Stream stderr for progress updates if a channel is provided.
    let stderr = child.stderr.take();
    let progress_user = progress_tx.as_ref().map(|(_, u)| u.to_string());
    let progress_sender = progress_tx.map(|(tx, _)| tx.clone());

    let stderr_task = tokio::spawn(async move {
        let mut collected = String::new();
        let Some(stderr) = stderr else {
            return collected;
        };
        let reader = tokio::io::BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            collected.push_str(&line);
            collected.push('\n');
            if let (Some(tx), Some(user)) = (&progress_sender, &progress_user)
                && let Some(msg) = parse_nix_progress(&line)
            {
                let _ = tx
                    .send(AgentEvent::Progress {
                        username: user.clone(),
                        percent: 50,
                        message: msg,
                    })
                    .await;
            }
        }
        collected
    });

    tokio::select! {
        status = child.wait() => {
            let stderr_output = stderr_task.await.unwrap_or_default();
            match status {
                Ok(s) if s.success() => Ok(()),
                Ok(_) => Err(format!("{command} failed: {stderr_output}")),
                Err(e) => Err(format!("failed to wait on {command}: {e}")),
            }
        }
        () = shutdown.cancelled() => {
            let _ = child.kill().await;
            Err("shutdown during preparation".into())
        }
    }
}

/// Collect disk usage summary via `df -h` for diagnostics.
async fn get_disk_usage_summary() -> String {
    match tokio::process::Command::new("df")
        .args(["-h", "/", "/nix/store", "/tmp"])
        .output()
        .await
    {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            // Deduplicate lines (/ and /nix/store are often the same mount)
            let mut seen = std::collections::HashSet::new();
            stdout
                .lines()
                .filter(|l| seen.insert(l.to_string()))
                .collect::<Vec<_>>()
                .join("\n")
        }
        Err(e) => format!("(failed to run df: {e})"),
    }
}

/// Extract the path count from "these N paths will be fetched (X MiB download, Y MiB unpacked):"
fn parse_paths_to_fetch_count(line: &str) -> Option<usize> {
    let line = line.trim();
    if line.starts_with("these ") && line.contains("paths will be fetched") {
        line.split_whitespace().nth(1)?.parse().ok()
    } else {
        None
    }
}

/// Extract the package name from a "copying path '...'" line.
fn parse_fetch_name(line: &str) -> Option<String> {
    let line = line.trim();
    if !line.starts_with("copying path '") {
        return None;
    }
    let path = line.strip_prefix("copying path '")?.split('\'').next()?;
    let basename = path.rsplit('/').next().unwrap_or(path);
    let name = if basename.len() > 33 { &basename[33..] } else { basename };
    Some(name.to_string())
}

/// Parse a non-fetch line of nix stderr into a progress message.
fn parse_nix_progress(line: &str) -> Option<String> {
    let line = line.trim();
    // Fetch lines handled by parse_fetch_name
    if line.starts_with("copying path '") {
        return None;
    }
    if line.starts_with("building '/nix/store/") {
        let path = line
            .strip_prefix("building '/nix/store/")?
            .split('\'')
            .next()?;
        // path is like "abc123hash-name.drv" — skip the 32-char hash prefix
        let name = if path.len() > 33 { &path[33..] } else { path };
        let name = name.strip_suffix(".drv").unwrap_or(name);
        Some(format!("building {name}"))
    } else if line.starts_with("these ") && line.contains("will be built") {
        // "these 42 derivations will be built:"
        let count = line.split_whitespace().nth(1)?;
        Some(format!("{count} derivations to build"))
    } else if line.starts_with("these ") && line.contains("will be fetched") {
        let count = line.split_whitespace().nth(1)?;
        Some(format!("{count} paths to fetch"))
    } else {
        None
    }
}

/// Read a single dconf key as a specific user via `runuser`.
///
/// Returns `None` if the key is unset or the command fails.
async fn read_dconf_as_user(username: &str, key: &str) -> Option<String> {
    let output = tokio::process::Command::new("runuser")
        .args(["-u", username, "--", "dconf", "read", key])
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if val.is_empty() || val == "@as []" {
        None
    } else {
        Some(val)
    }
}

/// Read the curated set of desktop preferences from dconf for a user.
async fn read_desktop_prefs(username: &str) -> DesktopPreferences {
    let favorite_apps = read_dconf_as_user(username, "/org/gnome/shell/favorite-apps")
        .await
        .and_then(|v| parse_dconf_string_array(&v));

    let wallpaper_uri =
        read_dconf_as_user(username, "/org/gnome/desktop/background/picture-uri-dark")
            .await
            .or(
                read_dconf_as_user(username, "/org/gnome/desktop/background/picture-uri").await,
            )
            .map(|v| strip_dconf_string(&v));

    let wallpaper_color =
        read_dconf_as_user(username, "/org/gnome/desktop/background/primary-color")
            .await
            .map(|v| strip_dconf_string(&v));

    let dark_mode =
        read_dconf_as_user(username, "/org/gnome/desktop/interface/color-scheme")
            .await
            .map(|v| v.contains("prefer-dark"));

    DesktopPreferences {
        favorite_apps,
        wallpaper_uri,
        wallpaper_color,
        dark_mode,
    }
}

/// Parse a dconf string array like `['firefox.desktop', 'org.gnome.Nautilus.desktop']`
/// into a `Vec<String>`.
fn parse_dconf_string_array(val: &str) -> Option<Vec<String>> {
    let trimmed = val.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return None;
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    if inner.trim().is_empty() {
        return Some(Vec::new());
    }
    let items: Vec<String> = inner
        .split(',')
        .map(|s| strip_dconf_string(s.trim()))
        .collect();
    Some(items)
}

/// Strip surrounding single quotes from a dconf string value.
fn strip_dconf_string(val: &str) -> String {
    val.trim()
        .trim_start_matches('\'')
        .trim_end_matches('\'')
        .to_string()
}

/// Sync observed desktop preferences back to the control plane for a user.
///
/// Reads the curated dconf keys from the user's session and sends them to
/// the machine-scoped desktop prefs endpoint.
pub async fn sync_user_desktop_prefs<C: HearthApiClient>(
    client: &C,
    machine_id: Uuid,
    username: &str,
) {
    let prefs = read_desktop_prefs(username).await;

    // Only sync if we got at least one meaningful value.
    if prefs.favorite_apps.is_none()
        && prefs.wallpaper_uri.is_none()
        && prefs.wallpaper_color.is_none()
        && prefs.dark_mode.is_none()
    {
        debug!(%username, "no desktop preferences to sync");
        return;
    }

    let req = SyncDesktopPrefsRequest { desktop: prefs };
    match client.sync_desktop_prefs(machine_id, username, &req).await {
        Ok(()) => {
            info!(%username, "synced desktop preferences to control plane");
        }
        Err(e) => {
            warn!(%username, error = %e, "failed to sync desktop preferences");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_dconf_string_array, parse_fetch_name, parse_nix_progress,
        parse_paths_to_fetch_count, strip_dconf_string,
    };

    #[test]
    fn test_copying_path() {
        let line = "copying path '/nix/store/aaaabbbbccccddddeeeeffffgggghhhh-hello-2.12.1' from 'https://cache.nixos.org'...";
        assert_eq!(parse_fetch_name(line), Some("hello-2.12.1".into()));
        // parse_nix_progress should NOT match fetch lines (handled separately)
        assert_eq!(parse_nix_progress(line), None);
    }

    #[test]
    fn test_copying_path_no_source() {
        let line = "copying path '/nix/store/aaaabbbbccccddddeeeeffffgggghhhh-glibc-2.40'";
        assert_eq!(parse_fetch_name(line), Some("glibc-2.40".into()));
    }

    #[test]
    fn test_paths_to_fetch_count() {
        let line = "these 157 paths will be fetched (312.50 MiB download, 1024.00 MiB unpacked):";
        assert_eq!(parse_paths_to_fetch_count(line), Some(157));
    }

    #[test]
    fn test_paths_to_fetch_count_no_match() {
        assert_eq!(parse_paths_to_fetch_count("building something..."), None);
    }

    #[test]
    fn test_building_derivation() {
        let line =
            "building '/nix/store/aaaabbbbccccddddeeeeffffgggghhhh-home-manager-path.drv'...";
        assert_eq!(
            parse_nix_progress(line),
            Some("building home-manager-path".into())
        );
    }

    #[test]
    fn test_building_multi_hyphen() {
        let line = "building '/nix/store/aaaabbbbccccddddeeeeffffgggghhhh-my-cool-package-1.0.drv'";
        assert_eq!(
            parse_nix_progress(line),
            Some("building my-cool-package-1.0".into())
        );
    }

    #[test]
    fn test_derivations_to_build() {
        let line = "these 42 derivations will be built:";
        assert_eq!(
            parse_nix_progress(line),
            Some("42 derivations to build".into())
        );
    }

    #[test]
    fn test_paths_to_fetch() {
        let line = "these 157 paths will be fetched (312.50 MiB download, 1024.00 MiB unpacked):";
        assert_eq!(parse_nix_progress(line), Some("157 paths to fetch".into()));
    }

    #[test]
    fn test_irrelevant_line() {
        assert_eq!(parse_nix_progress("evaluating derivation..."), None);
        assert_eq!(parse_nix_progress(""), None);
        assert_eq!(parse_nix_progress("warning: Git tree is dirty"), None);
    }

    // --- dconf parsing tests ---

    #[test]
    fn test_parse_dconf_string_array_typical() {
        let val = "['firefox.desktop', 'org.gnome.Nautilus.desktop', 'kitty.desktop']";
        let result = parse_dconf_string_array(val).unwrap();
        assert_eq!(
            result,
            vec![
                "firefox.desktop",
                "org.gnome.Nautilus.desktop",
                "kitty.desktop"
            ]
        );
    }

    #[test]
    fn test_parse_dconf_string_array_empty() {
        assert_eq!(parse_dconf_string_array("[]"), Some(vec![]));
        assert_eq!(parse_dconf_string_array("[  ]"), Some(vec![]));
    }

    #[test]
    fn test_parse_dconf_string_array_single() {
        let val = "['firefox.desktop']";
        assert_eq!(
            parse_dconf_string_array(val),
            Some(vec!["firefox.desktop".to_string()])
        );
    }

    #[test]
    fn test_parse_dconf_string_array_invalid() {
        assert_eq!(parse_dconf_string_array("not-an-array"), None);
        assert_eq!(parse_dconf_string_array(""), None);
        assert_eq!(parse_dconf_string_array("@as []"), None);
    }

    #[test]
    fn test_strip_dconf_string() {
        assert_eq!(strip_dconf_string("'hello'"), "hello");
        assert_eq!(strip_dconf_string("  'spaced'  "), "spaced");
        assert_eq!(strip_dconf_string("no-quotes"), "no-quotes");
        assert_eq!(strip_dconf_string("'prefer-dark'"), "prefer-dark");
    }

    #[test]
    fn test_desktop_preferences_serialization_roundtrip() {
        use hearth_common::api_types::DesktopPreferences;

        let prefs = DesktopPreferences {
            favorite_apps: Some(vec![
                "firefox.desktop".into(),
                "org.gnome.Nautilus.desktop".into(),
            ]),
            wallpaper_uri: Some("file:///usr/share/backgrounds/gnome/blobs-l.svg".into()),
            wallpaper_color: Some("#1e1e2e".into()),
            dark_mode: Some(true),
        };

        let json = serde_json::to_string(&prefs).unwrap();
        let deserialized: DesktopPreferences = serde_json::from_str(&json).unwrap();
        assert_eq!(prefs, deserialized);
    }

    #[test]
    fn test_desktop_preferences_partial_serialization() {
        use hearth_common::api_types::DesktopPreferences;

        // Only favorite_apps set — others should be absent in JSON.
        let prefs = DesktopPreferences {
            favorite_apps: Some(vec!["firefox.desktop".into()]),
            wallpaper_uri: None,
            wallpaper_color: None,
            dark_mode: None,
        };

        let json = serde_json::to_string(&prefs).unwrap();
        assert!(!json.contains("wallpaper_uri"));
        assert!(!json.contains("wallpaper_color"));
        assert!(!json.contains("dark_mode"));

        let deserialized: DesktopPreferences = serde_json::from_str(&json).unwrap();
        assert_eq!(prefs, deserialized);
    }

    #[test]
    fn test_desktop_preferences_empty_roundtrip() {
        use hearth_common::api_types::DesktopPreferences;

        let prefs = DesktopPreferences {
            favorite_apps: None,
            wallpaper_uri: None,
            wallpaper_color: None,
            dark_mode: None,
        };

        let json = serde_json::to_string(&prefs).unwrap();
        assert_eq!(json, "{}");

        let deserialized: DesktopPreferences = serde_json::from_str(&json).unwrap();
        assert_eq!(prefs, deserialized);
    }
}

/// Serialize an [`AgentEvent`] as a single JSON line and write it to the stream.
async fn send_event(
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    event: &AgentEvent,
) -> std::io::Result<()> {
    let mut payload = serde_json::to_string(event)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    payload.push('\n');
    writer.write_all(payload.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}
