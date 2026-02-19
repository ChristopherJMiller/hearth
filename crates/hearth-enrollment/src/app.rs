use crossterm::event::KeyEvent;
use ratatui::prelude::*;
use uuid::Uuid;

use crate::screens::enroll::EnrollScreen;
use crate::screens::hardware::HardwareScreen;
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Welcome,
    Hardware,
    Network,
    Enroll,
    Status,
    Provisioning,
    Done,
}

pub struct App {
    screen: Screen,
    data: EnrollmentData,
    welcome: WelcomeScreen,
    hardware: HardwareScreen,
    network: NetworkScreen,
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
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::Welcome,
            data: EnrollmentData::default(),
            welcome: WelcomeScreen::new(),
            hardware: HardwareScreen::new(),
            network: NetworkScreen::new(),
            enroll: EnrollScreen::new(),
            status: StatusScreen::new(),
            provision: ProvisionScreen::new(),
            hw_detected: false,
            net_checked: false,
            polling_started: false,
            provisioning_started: false,
        }
    }

    pub fn render(&self, frame: &mut Frame) {
        match self.screen {
            Screen::Welcome => self.welcome.render(frame),
            Screen::Hardware => self.hardware.render(frame, &self.data),
            Screen::Network => self.network.render(frame, &self.data),
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
                }
            }
            Screen::Hardware => {
                if !self.hw_detected {
                    self.hardware.detect(&mut self.data);
                    self.hw_detected = true;
                }
                if self.hardware.handle_key(key) {
                    self.screen = Screen::Network;
                }
            }
            Screen::Network => {
                if !self.net_checked {
                    self.network.check(&mut self.data);
                    self.net_checked = true;
                }
                if self.network.handle_key(key) {
                    self.screen = Screen::Enroll;
                }
            }
            Screen::Enroll => {
                if let Some(advance) = self.enroll.handle_key(key, &mut self.data).await
                    && advance
                {
                    self.screen = Screen::Status;
                }
            }
            Screen::Status => {
                if self.status.handle_key(key) {
                    // Transfer the target closure and cache credentials captured
                    // during approval into enrollment data so the provision screen
                    // gets them.
                    if let Some(closure) = self.status.take_approved_closure() {
                        self.data.target_closure = Some(closure);
                    }
                    let (url, token) = self.status.take_cache_credentials();
                    if url.is_some() {
                        self.data.cache_url = url;
                    }
                    self.data.cache_token = token;
                    self.screen = Screen::Provisioning;
                }
            }
            Screen::Provisioning => {
                if self.provision.handle_key(key).await {
                    self.screen = Screen::Done;
                }
            }
            Screen::Done => {}
        }
    }

    pub async fn tick(&mut self) {
        match self.screen {
            Screen::Hardware => {
                if !self.hw_detected {
                    self.hardware.detect(&mut self.data);
                    self.hw_detected = true;
                }
            }
            Screen::Network => {
                if !self.net_checked {
                    self.network.check(&mut self.data);
                    self.net_checked = true;
                }
            }
            Screen::Status => {
                if !self.polling_started {
                    self.status.start_polling(&self.data);
                    self.polling_started = true;
                }
                let approved = self.status.tick(&self.data).await;
                if approved {
                    // Stay on Status screen so user can see the approval message
                    // and press Enter to proceed to provisioning.
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
                }
            }
            _ => {}
        }
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
}
