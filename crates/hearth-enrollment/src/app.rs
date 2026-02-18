use crossterm::event::KeyEvent;
use ratatui::prelude::*;
use uuid::Uuid;

use crate::screens::enroll::EnrollScreen;
use crate::screens::hardware::HardwareScreen;
use crate::screens::network::NetworkScreen;
use crate::screens::status::StatusScreen;
use crate::screens::welcome::WelcomeScreen;

pub type AppResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Data that flows through the enrollment wizard.
#[derive(Default)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Welcome,
    Hardware,
    Network,
    Enroll,
    Status,
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
    /// Set to true once hardware detection has been triggered for the current visit.
    hw_detected: bool,
    /// Set to true once network check has been triggered for the current visit.
    net_checked: bool,
    /// Set to true once status polling has been started.
    polling_started: bool,
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
            hw_detected: false,
            net_checked: false,
            polling_started: false,
        }
    }

    pub fn render(&self, frame: &mut Frame) {
        match self.screen {
            Screen::Welcome => self.welcome.render(frame),
            Screen::Hardware => self.hardware.render(frame, &self.data),
            Screen::Network => self.network.render(frame, &self.data),
            Screen::Enroll => self.enroll.render(frame),
            Screen::Status => self.status.render(frame),
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
                    // and press Enter to exit.
                }
            }
            _ => {}
        }
    }

    pub fn should_exit(&self) -> bool {
        self.screen == Screen::Done
    }

    /// Whether the user is allowed to quit with 'q' right now.
    /// Disallow during enrollment submission or status polling to prevent
    /// accidental exits mid-flow, but the status screen itself allows 'q'.
    pub fn can_quit(&self) -> bool {
        matches!(
            self.screen,
            Screen::Welcome | Screen::Hardware | Screen::Network | Screen::Status
        )
    }
}
