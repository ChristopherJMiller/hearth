use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use std::time::Instant;
use tracing::{error, info};

use crate::app::EnrollmentData;
use crate::ui;

use hearth_common::api_client::{HearthApiClient, ReqwestApiClient};
use hearth_common::api_types::EnrollmentStatus;

#[derive(Debug)]
enum PollStatus {
    Waiting,
    Approved,
    Error(String),
}

pub struct StatusScreen {
    status: PollStatus,
    last_poll: Option<Instant>,
    client: Option<ReqwestApiClient>,
    machine_id: Option<uuid::Uuid>,
    dots: usize,
    /// Captured from the approval response so provisioning can use it immediately.
    approved_closure: Option<String>,
    /// Cache credentials from extra_config for authenticated cache access.
    cache_url: Option<String>,
    cache_token: Option<String>,
    /// Machine auth token for the agent, received after approval.
    machine_token: Option<String>,
    /// Disko config name for disk partitioning during provisioning.
    disko_config: Option<String>,
}

impl StatusScreen {
    pub fn new() -> Self {
        Self {
            status: PollStatus::Waiting,
            last_poll: None,
            client: None,
            machine_id: None,
            dots: 0,
            approved_closure: None,
            cache_url: None,
            cache_token: None,
            machine_token: None,
            disko_config: None,
        }
    }

    /// Returns the target closure captured at approval time, if any.
    pub fn take_approved_closure(&mut self) -> Option<String> {
        self.approved_closure.take()
    }

    /// Returns cache credentials (url, token) captured from extra_config at approval time.
    pub fn take_cache_credentials(&mut self) -> (Option<String>, Option<String>) {
        (self.cache_url.take(), self.cache_token.take())
    }

    /// Returns the machine token received after approval.
    pub fn take_machine_token(&mut self) -> Option<String> {
        self.machine_token.take()
    }

    /// Returns the disko config name for disk partitioning.
    pub fn take_disko_config(&mut self) -> Option<String> {
        self.disko_config.take()
    }

    pub fn start_polling(&mut self, data: &EnrollmentData) {
        // If the device has a user token, use it for authenticated polling.
        let client = match &data.user_token {
            Some(token) => ReqwestApiClient::new_with_token(data.server_url.clone(), token.clone()),
            None => ReqwestApiClient::new(data.server_url.clone()),
        };
        self.client = Some(client);
        self.machine_id = data.machine_id;
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let center = ui::centered_rect(60, 40, area);
        let block = ui::hearth_block(" Awaiting Approval ");

        let inner = block.inner(center);
        frame.render_widget(block, center);

        match &self.status {
            PollStatus::Waiting => {
                let dots_str = ".".repeat((self.dots % 4) + 1);
                let items = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("  Waiting for admin approval{dots_str}"),
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
                        "  An administrator must approve this device",
                        Style::default().fg(ui::MUTED),
                    )),
                    Line::from(Span::styled(
                        "  before enrollment can proceed.",
                        Style::default().fg(ui::MUTED),
                    )),
                    Line::from(""),
                    Line::from(Span::styled("  q to quit", Style::default().fg(ui::MUTED))),
                ];
                frame.render_widget(Paragraph::new(items), inner);
            }
            PollStatus::Approved => {
                let items = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Device approved!",
                        Style::default().fg(Color::Green).bold(),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Enrollment complete. The system will now provision.",
                        Style::default().fg(Color::White),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Press Enter to exit",
                        Style::default().fg(ui::MUTED),
                    )),
                ];
                frame.render_widget(Paragraph::new(items), inner);
            }
            PollStatus::Error(err) => {
                let items = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Poll error",
                        Style::default().fg(Color::Red).bold(),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("  {err}"),
                        Style::default().fg(Color::Red),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Will retry automatically...",
                        Style::default().fg(ui::MUTED),
                    )),
                ];
                frame.render_widget(Paragraph::new(items), inner);
            }
        }
    }

    /// Returns true if approved (signals exit).
    pub async fn tick(&mut self, _data: &EnrollmentData) -> bool {
        self.dots += 1;

        // Poll every 3 seconds
        let should_poll = match self.last_poll {
            Some(last) => last.elapsed().as_secs() >= 3,
            None => true,
        };

        if !should_poll {
            return false;
        }

        if let (Some(client), Some(machine_id)) = (&self.client, self.machine_id) {
            self.last_poll = Some(Instant::now());
            match client.get_enrollment_status(machine_id).await {
                Ok(resp) => match resp.status {
                    EnrollmentStatus::Pending => {
                        self.status = PollStatus::Waiting;
                    }
                    EnrollmentStatus::Approved
                    | EnrollmentStatus::Enrolled
                    | EnrollmentStatus::Provisioning
                    | EnrollmentStatus::Active => {
                        info!("device approved!");
                        // Capture the machine token (minted by the server on first
                        // status poll after approval).
                        if self.machine_token.is_none() {
                            self.machine_token = resp.machine_token;
                            if self.machine_token.is_some() {
                                info!("received machine auth token from control plane");
                            }
                        }
                        // Capture provisioning data from the extended response.
                        if self.approved_closure.is_none() {
                            self.approved_closure = resp.target_closure;
                        }
                        if self.cache_url.is_none() {
                            self.cache_url = resp.cache_url;
                        }
                        if self.cache_token.is_none() {
                            self.cache_token = resp.cache_token;
                        }
                        if self.disko_config.is_none() {
                            self.disko_config = resp.disko_config;
                        }
                        self.status = PollStatus::Approved;
                    }
                    EnrollmentStatus::Decommissioned => {
                        self.status = PollStatus::Error("Device was decommissioned".into());
                    }
                },
                Err(e) => {
                    error!(error = %e, "failed to poll enrollment status");
                    self.status = PollStatus::Error(e.to_string());
                }
            }
        }

        matches!(self.status, PollStatus::Approved)
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        matches!(self.status, PollStatus::Approved) && matches!(key.code, KeyCode::Enter)
    }
}
