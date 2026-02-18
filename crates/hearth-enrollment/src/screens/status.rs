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
}

impl StatusScreen {
    pub fn new() -> Self {
        Self {
            status: PollStatus::Waiting,
            last_poll: None,
            client: None,
            machine_id: None,
            dots: 0,
        }
    }

    pub fn start_polling(&mut self, data: &EnrollmentData) {
        self.client = Some(ReqwestApiClient::new(data.server_url.clone()));
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
                Ok(machine) => match machine.enrollment_status {
                    EnrollmentStatus::Pending => {
                        self.status = PollStatus::Waiting;
                    }
                    EnrollmentStatus::Approved
                    | EnrollmentStatus::Enrolled
                    | EnrollmentStatus::Provisioning
                    | EnrollmentStatus::Active => {
                        info!("device approved!");
                        self.status = PollStatus::Approved;
                        // Don't auto-exit, let user see the message
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
