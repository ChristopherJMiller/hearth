use std::fmt;

use crossterm::event::KeyEvent;
use ratatui::prelude::*;
use tracing::info;
use uuid::Uuid;

/// Runtime directory for enrollment state files, used by VM test harnesses.
const RUNTIME_DIR: &str = "/run/hearth";

use crate::screens::enroll::EnrollScreen;
use crate::screens::hardware::HardwareScreen;
use crate::screens::login::LoginScreen;
use crate::screens::network::NetworkScreen;
use crate::screens::provision::ProvisionScreen;
use crate::screens::status::StatusScreen;
use crate::screens::welcome::WelcomeScreen;

pub type AppResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Data that flows through the enrollment wizard.
#[derive(Default)]
#[allow(dead_code)]
pub struct EnrollmentData {
    pub hostname: String,
    pub cpu: String,
    pub ram: String,
    pub disk: String,
    pub nic: String,
    pub ip_address: String,
    pub hardware_fingerprint: Option<String>,
    pub server_url: String,
    pub machine_id: Option<Uuid>,
    pub target_closure: Option<String>,
    pub cache_url: Option<String>,
    pub cache_token: Option<String>,
    pub target_disk: Option<String>,
    /// User OIDC access token from device flow login.
    pub user_token: Option<String>,
    /// Kanidm URL used for authentication.
    pub kanidm_url: Option<String>,
    /// Machine auth token received after enrollment approval.
    pub machine_token: Option<String>,
    /// Disko config name for disk partitioning (e.g., "standard", "luks-lvm").
    pub disko_config: Option<String>,
    /// Device serial number for asset tracking.
    pub serial_number: Option<String>,
    /// Generated NixOS hardware-configuration.nix content from the device.
    pub hardware_config: Option<String>,
    /// Headscale pre-auth key for mesh VPN join on first boot.
    pub headscale_preauth_key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Welcome,
    Hardware,
    Network,
    Login,
    Enroll,
    Status,
    Provisioning,
    Done,
}

impl fmt::Display for Screen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Welcome => "welcome",
            Self::Hardware => "hardware",
            Self::Network => "network",
            Self::Login => "login",
            Self::Enroll => "enroll",
            Self::Status => "status",
            Self::Provisioning => "provisioning",
            Self::Done => "done",
        };
        f.write_str(name)
    }
}

pub struct App {
    screen: Screen,
    data: EnrollmentData,
    welcome: WelcomeScreen,
    hardware: HardwareScreen,
    network: NetworkScreen,
    login: LoginScreen,
    enroll: EnrollScreen,
    status: StatusScreen,
    provision: ProvisionScreen,
    /// Set to true once hardware detection has been triggered for the current visit.
    hw_detected: bool,
    /// Set to true once network check has been triggered for the current visit.
    net_checked: bool,
    /// Set to true once status polling has been started.
    polling_started: bool,
    /// Set to true once provisioning has been started.
    provisioning_started: bool,
    /// When true, auto-advance through screens without waiting for Enter keypresses.
    /// Set from `HEARTH_HEADLESS=1` env var.
    headless: bool,
    /// Tracks whether login tick returned authenticated (for headless auto-advance).
    login_authenticated: bool,
    /// Whether the machine_id file has been written (avoid repeated writes every tick).
    machine_id_written: bool,
}

impl App {
    pub fn new() -> Self {
        let headless = std::env::var("HEARTH_HEADLESS")
            .map(|v| v == "1")
            .unwrap_or(false);
        if headless {
            info!("headless mode enabled");
        }
        Self {
            screen: Screen::Welcome,
            data: EnrollmentData::default(),
            welcome: WelcomeScreen::new(),
            hardware: HardwareScreen::new(),
            network: NetworkScreen::new(),
            login: LoginScreen::new(),
            enroll: EnrollScreen::new(),
            status: StatusScreen::new(),
            provision: ProvisionScreen::new(),
            hw_detected: false,
            net_checked: false,
            polling_started: false,
            provisioning_started: false,
            headless,
            login_authenticated: false,
            machine_id_written: false,
        }
    }

    pub fn render(&self, frame: &mut Frame) {
        match self.screen {
            Screen::Welcome => self.welcome.render(frame),
            Screen::Hardware => self.hardware.render(frame, &self.data),
            Screen::Network => self.network.render(frame, &self.data),
            Screen::Login => self.login.render(frame),
            Screen::Enroll => self.enroll.render(frame),
            Screen::Status => self.status.render(frame),
            Screen::Provisioning => self.provision.render(frame),
            Screen::Done => {}
        }
    }

    pub async fn handle_key(&mut self, key: KeyEvent) {
        match self.screen {
            Screen::Welcome => {
                if self.welcome.handle_key(key) {
                    self.screen = Screen::Hardware;
                    self.write_state_file();
                }
            }
            Screen::Hardware => {
                if !self.hw_detected {
                    self.hardware.detect(&mut self.data);
                    self.hw_detected = true;
                }
                if self.hardware.handle_key(key) {
                    self.screen = Screen::Network;
                    self.write_state_file();
                }
            }
            Screen::Network => {
                if !self.net_checked {
                    self.network.check(&mut self.data);
                    self.net_checked = true;
                }
                if self.network.handle_key(key) {
                    self.screen = Screen::Login;
                    self.write_state_file();
                }
            }
            Screen::Login => {
                if self.login.handle_key(key) {
                    self.screen = Screen::Enroll;
                    self.write_state_file();
                }
            }
            Screen::Enroll => {
                if let Some(advance) = self.enroll.handle_key(key, &mut self.data).await
                    && advance
                {
                    self.screen = Screen::Status;
                    self.write_state_file();
                }
            }
            Screen::Status => {
                if self.status.handle_key(key) {
                    self.transfer_status_to_provisioning();
                    self.screen = Screen::Provisioning;
                    self.write_state_file();
                }
            }
            Screen::Provisioning => {
                if self.provision.handle_key(key).await {
                    self.screen = Screen::Done;
                    self.write_state_file();
                }
            }
            Screen::Done => {}
        }
    }

    /// Transfer approval data from the status screen into enrollment data
    /// for the provisioning screen. Used by both handle_key and headless auto-advance.
    fn transfer_status_to_provisioning(&mut self) {
        if let Some(closure) = self.status.take_approved_closure() {
            self.data.target_closure = Some(closure);
        }
        let (url, token) = self.status.take_cache_credentials();
        if url.is_some() {
            self.data.cache_url = url;
        }
        self.data.cache_token = token;
        self.data.machine_token = self.status.take_machine_token();
        self.data.disko_config = self.status.take_disko_config();
        self.data.headscale_preauth_key = self.status.take_headscale_preauth_key();
    }

    pub async fn tick(&mut self) {
        match self.screen {
            Screen::Welcome => {
                if self.headless {
                    self.screen = Screen::Hardware;
                    self.write_state_file();
                    // Trigger hw detection immediately
                    self.hardware.detect(&mut self.data);
                    self.hw_detected = true;
                }
            }
            Screen::Hardware => {
                if !self.hw_detected {
                    self.hardware.detect(&mut self.data);
                    self.hw_detected = true;
                }
                if self.headless && self.hw_detected {
                    self.screen = Screen::Network;
                    self.write_state_file();
                    // Trigger network check immediately
                    self.network.check(&mut self.data);
                    self.net_checked = true;
                }
            }
            Screen::Network => {
                if !self.net_checked {
                    self.network.check(&mut self.data);
                    self.net_checked = true;
                }
                if self.headless && self.net_checked {
                    self.screen = Screen::Login;
                    self.write_state_file();
                }
            }
            Screen::Login => {
                let authenticated = self.login.tick(&mut self.data).await;
                if authenticated {
                    self.login_authenticated = true;
                }
                if self.headless && self.login_authenticated {
                    self.screen = Screen::Enroll;
                    self.write_state_file();
                }
            }
            Screen::Enroll => {
                self.enroll.tick(&mut self.data).await;
                // Write machine_id file once when enrollment succeeds
                if let Some(id) = self.data.machine_id
                    && !self.machine_id_written
                {
                    let _ = std::fs::create_dir_all(RUNTIME_DIR);
                    let _ = std::fs::write(
                        format!("{RUNTIME_DIR}/enrollment-machine-id"),
                        id.to_string(),
                    );
                    self.machine_id_written = true;
                }
                if self.headless && self.enroll.is_success() {
                    self.screen = Screen::Status;
                    self.write_state_file();
                }
            }
            Screen::Status => {
                if !self.polling_started {
                    self.status.start_polling(&self.data);
                    self.polling_started = true;
                }
                let approved = self.status.tick(&self.data).await;
                if approved && self.headless {
                    self.transfer_status_to_provisioning();
                    self.screen = Screen::Provisioning;
                    self.write_state_file();
                }
            }
            Screen::Provisioning => {
                if !self.provisioning_started {
                    self.provision.start(&self.data);
                    self.provisioning_started = true;
                }
                let done = self.provision.tick().await;
                if done {
                    self.screen = Screen::Done;
                    self.write_state_file();
                }
            }
            _ => {}
        }
    }

    /// Write the current screen name to `/run/hearth/enrollment-state` for observability.
    fn write_state_file(&self) {
        let name = self.screen.to_string();
        let _ = std::fs::create_dir_all(RUNTIME_DIR);
        let _ = std::fs::write(format!("{RUNTIME_DIR}/enrollment-state"), &name);
        info!(event = "screen_transition", to = name);
    }

    pub fn should_exit(&self) -> bool {
        self.screen == Screen::Done
    }

    /// Whether the user is allowed to quit with 'q' right now.
    /// Disallow during enrollment submission or active provisioning to prevent
    /// accidental exits mid-flow, but the status screen itself allows 'q'.
    pub fn can_quit(&self) -> bool {
        match self.screen {
            Screen::Welcome | Screen::Hardware | Screen::Network | Screen::Status => true,
            Screen::Provisioning => self.provision.can_quit(),
            _ => false,
        }
    }

    /// Take a pending browser request from the login screen, if any.
    /// The main loop uses this to suspend the TUI and launch a kiosk browser.
    /// Returns (auth_url, callback_signal) — the main loop closes the browser
    /// as soon as the signal fires.
    pub fn take_browser_request(&mut self) -> Option<(String, tokio::sync::oneshot::Receiver<()>)> {
        self.login.take_browser_request()
    }

    /// Notify the login screen that the browser failed to launch.
    pub fn notify_browser_failed(&mut self, err: String) {
        self.login.notify_browser_failed(err);
    }
}
