use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use tracing::{error, info};

use crate::app::EnrollmentData;
use crate::ui;

use hearth_common::api_client::{HearthApiClient, ReqwestApiClient};
use hearth_common::api_types::EnrollmentRequest;

#[derive(Debug)]
enum EnrollState {
    /// Auto-submitting with the pre-configured URL (no user input shown).
    AutoSubmitting,
    Input,
    Submitting,
    Success(String), // message
    Error(String),
}

pub struct EnrollScreen {
    url_input: String,
    cursor_pos: usize,
    state: EnrollState,
    /// Whether the auto-submit has been kicked off (to avoid re-triggering).
    auto_submitted: bool,
}

impl EnrollScreen {
    pub fn new() -> Self {
        let preconfigured = std::env::var("HEARTH_SERVER_URL")
            .ok()
            .filter(|s| s.len() > 7);
        let (url_input, initial_state) = match preconfigured {
            Some(url) => (url, EnrollState::AutoSubmitting),
            None => ("http://".to_string(), EnrollState::Input),
        };
        let cursor_pos = url_input.len();
        Self {
            url_input,
            cursor_pos,
            state: initial_state,
            auto_submitted: false,
        }
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let center = ui::centered_rect(70, 50, area);
        let block = ui::hearth_block(" Enrollment ");

        let inner = block.inner(center);
        frame.render_widget(block, center);

        match &self.state {
            EnrollState::AutoSubmitting => {
                let items = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("  Connecting to {}...", self.url_input),
                        Style::default().fg(Color::Yellow),
                    )),
                ];
                frame.render_widget(Paragraph::new(items), inner);
            }
            EnrollState::Input => {
                let items = vec![
                    Line::from(Span::styled(
                        "  Enter the Hearth control plane URL:",
                        Style::default().fg(Color::White),
                    )),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("  > ", Style::default().fg(ui::EMBER)),
                        Span::styled(&self.url_input, Style::default().fg(Color::White).bold()),
                        Span::styled("_", Style::default().fg(ui::EMBER)),
                    ]),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Press Enter to submit  |  Esc to go back",
                        Style::default().fg(ui::MUTED),
                    )),
                ];
                frame.render_widget(Paragraph::new(items), inner);
            }
            EnrollState::Submitting => {
                let items = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Submitting enrollment request...",
                        Style::default().fg(Color::Yellow),
                    )),
                ];
                frame.render_widget(Paragraph::new(items), inner);
            }
            EnrollState::Success(msg) => {
                let items = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Enrollment submitted!",
                        Style::default().fg(Color::Green).bold(),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("  {msg}"),
                        Style::default().fg(Color::White),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Press Enter to continue",
                        Style::default().fg(ui::MUTED),
                    )),
                ];
                frame.render_widget(Paragraph::new(items), inner);
            }
            EnrollState::Error(err) => {
                let items = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Enrollment failed",
                        Style::default().fg(Color::Red).bold(),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("  {err}"),
                        Style::default().fg(Color::Red),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Press Enter to retry  |  Esc to go back",
                        Style::default().fg(ui::MUTED),
                    )),
                ];
                frame.render_widget(Paragraph::new(items), inner);
            }
        }
    }

    /// Returns Some(true) to advance to status screen, Some(false) to stay, None for no change.
    pub async fn handle_key(&mut self, key: KeyEvent, data: &mut EnrollmentData) -> Option<bool> {
        match &self.state {
            EnrollState::AutoSubmitting | EnrollState::Submitting => None,
            EnrollState::Input => match key.code {
                KeyCode::Enter => {
                    if self.url_input.len() > 7 {
                        // Trim trailing slash
                        let url = self.url_input.trim_end_matches('/').to_string();
                        data.server_url = url;
                        self.state = EnrollState::Submitting;

                        // Do the enrollment
                        self.submit_enrollment(data).await;
                    }
                    Some(false)
                }
                KeyCode::Char(c) => {
                    self.url_input.insert(self.cursor_pos, c);
                    self.cursor_pos += 1;
                    Some(false)
                }
                KeyCode::Backspace => {
                    if self.cursor_pos > 0 {
                        self.cursor_pos -= 1;
                        self.url_input.remove(self.cursor_pos);
                    }
                    Some(false)
                }
                _ => None,
            },
            EnrollState::Success(_) => {
                if matches!(key.code, KeyCode::Enter) {
                    Some(true) // Advance to status screen
                } else {
                    None
                }
            }
            EnrollState::Error(_) => match key.code {
                KeyCode::Enter => {
                    self.state = EnrollState::Input;
                    Some(false)
                }
                _ => None,
            },
        }
    }

    /// Whether the enrollment is in the Success state (for headless auto-advance).
    pub fn is_success(&self) -> bool {
        matches!(self.state, EnrollState::Success(_))
    }

    /// Called on each tick. Kicks off auto-submit when HEARTH_SERVER_URL was set.
    pub async fn tick(&mut self, data: &mut EnrollmentData) {
        if matches!(self.state, EnrollState::AutoSubmitting) && !self.auto_submitted {
            self.auto_submitted = true;
            let url = self.url_input.trim_end_matches('/').to_string();
            data.server_url = url;
            self.submit_enrollment(data).await;
        }
    }

    async fn submit_enrollment(&mut self, data: &mut EnrollmentData) {
        // Use the user's OIDC token for authenticated enrollment if available.
        let client = match &data.user_token {
            Some(token) => ReqwestApiClient::new_with_token(data.server_url.clone(), token.clone()),
            None => ReqwestApiClient::new(data.server_url.clone()),
        };
        let hardware_report = Some(crate::hw::to_hardware_report(data));
        let req = EnrollmentRequest {
            hostname: data.hostname.clone(),
            hardware_fingerprint: data.hardware_fingerprint.clone(),
            os_version: None,
            role_hint: None,
            hardware_report,
            serial_number: data.serial_number.clone(),
            hardware_config: data.hardware_config.clone(),
        };

        match client.enroll(&req).await {
            Ok(resp) => {
                info!(machine_id = %resp.machine_id, "enrollment submitted");
                data.machine_id = Some(resp.machine_id);
                self.state = EnrollState::Success(format!(
                    "Machine ID: {}  --  Awaiting admin approval",
                    resp.machine_id
                ));
            }
            Err(e) => {
                error!(error = %e, "enrollment failed");
                self.state = EnrollState::Error(e.to_string());
            }
        }
    }
}
