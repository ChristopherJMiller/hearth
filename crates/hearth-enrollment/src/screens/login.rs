//! Login screen: OAuth2 Device Authorization Flow with QR code.
//!
//! Displays a verification URL + user code + QR code. The operator
//! scans the QR or visits the URL on their phone/laptop to authenticate
//! via Kanidm. Background polling waits for the token.

use crossterm::event::{KeyCode, KeyEvent};
use qrcode::QrCode;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use tracing::{error, info, warn};

use crate::app::EnrollmentData;
use crate::oauth::{self, DeviceFlowState, PollStatus};
use crate::ui;

#[derive(Debug)]
enum LoginState {
    /// Waiting for user to start the flow (initial state).
    Ready,
    /// Device flow started, waiting for user to authenticate.
    Waiting {
        flow: DeviceFlowState,
        qr_lines: Vec<String>,
        poll_interval: u64,
        elapsed_polls: u64,
    },
    /// Authentication succeeded, user token acquired.
    Authenticated { username: String },
    /// Something went wrong.
    Error(String),
}

pub struct LoginScreen {
    state: LoginState,
    /// Kanidm URL for the device flow.
    kanidm_url: String,
    /// OAuth2 client ID for enrollment.
    client_id: String,
    /// Whether the initial device flow has been kicked off.
    started: bool,
}

impl LoginScreen {
    pub fn new() -> Self {
        let kanidm_url =
            std::env::var("HEARTH_KANIDM_URL").unwrap_or_else(|_| "https://localhost:8443".into());
        let client_id =
            std::env::var("HEARTH_KANIDM_CLIENT_ID").unwrap_or_else(|_| "hearth-enrollment".into());
        Self {
            state: LoginState::Ready,
            kanidm_url,
            client_id,
            started: false,
        }
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let center = ui::centered_rect(80, 80, area);
        let block = ui::hearth_block(" Sign In ");

        let inner = block.inner(center);
        frame.render_widget(block, center);

        match &self.state {
            LoginState::Ready => {
                let items = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Authenticating with identity provider...",
                        Style::default().fg(Color::Yellow),
                    )),
                ];
                frame.render_widget(Paragraph::new(items), inner);
            }
            LoginState::Waiting {
                flow,
                qr_lines,
                elapsed_polls,
                ..
            } => {
                render_waiting(frame, inner, flow, qr_lines, *elapsed_polls);
            }
            LoginState::Authenticated { username } => {
                let items = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Authenticated!",
                        Style::default().fg(Color::Green).bold(),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("  Signed in as: {username}"),
                        Style::default().fg(Color::White),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Press Enter to continue to enrollment",
                        Style::default().fg(ui::MUTED),
                    )),
                ];
                frame.render_widget(Paragraph::new(items), inner);
            }
            LoginState::Error(err) => {
                let items = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Authentication failed",
                        Style::default().fg(Color::Red).bold(),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("  {err}"),
                        Style::default().fg(Color::Red),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Press Enter to retry",
                        Style::default().fg(ui::MUTED),
                    )),
                ];
                frame.render_widget(Paragraph::new(items), inner);
            }
        }
    }

    /// Returns true when login is complete and we should advance.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match &self.state {
            LoginState::Authenticated { .. } => {
                matches!(key.code, KeyCode::Enter)
            }
            LoginState::Error(_) => {
                if matches!(key.code, KeyCode::Enter) {
                    self.state = LoginState::Ready;
                    self.started = false;
                }
                false
            }
            _ => false,
        }
    }

    /// Called on each tick. Starts the device flow or polls for the token.
    pub async fn tick(&mut self, data: &mut EnrollmentData) -> bool {
        match &self.state {
            LoginState::Ready => {
                if !self.started {
                    self.started = true;
                    self.start_flow().await;
                }
                false
            }
            LoginState::Waiting { .. } => {
                self.poll_token(data).await;
                matches!(self.state, LoginState::Authenticated { .. })
            }
            LoginState::Authenticated { .. } => true,
            LoginState::Error(_) => false,
        }
    }

    async fn start_flow(&mut self) {
        match oauth::start_device_flow(&self.kanidm_url, &self.client_id).await {
            Ok(flow) => {
                let qr_url = flow
                    .verification_uri_complete
                    .as_deref()
                    .unwrap_or(&flow.verification_uri);
                let qr_lines = render_qr_to_lines(qr_url);
                let interval = flow.interval;
                self.state = LoginState::Waiting {
                    flow,
                    qr_lines,
                    poll_interval: interval,
                    elapsed_polls: 0,
                };
            }
            Err(e) => {
                error!(error = %e, "failed to start device flow");
                self.state = LoginState::Error(e);
            }
        }
    }

    async fn poll_token(&mut self, data: &mut EnrollmentData) {
        // Extract what we need without holding a mutable ref
        let (device_code, poll_interval, elapsed) = match &self.state {
            LoginState::Waiting {
                flow,
                poll_interval,
                elapsed_polls,
                ..
            } => (flow.device_code.clone(), *poll_interval, *elapsed_polls),
            _ => return,
        };

        // Only poll at the specified interval (tick is ~250ms, so count ticks)
        let ticks_per_poll = (poll_interval * 4).max(1);
        let new_elapsed = elapsed + 1;

        // Update the counter
        if let LoginState::Waiting { elapsed_polls, .. } = &mut self.state {
            *elapsed_polls = new_elapsed;
        }

        if new_elapsed % ticks_per_poll != 0 {
            return;
        }

        let status = oauth::poll_for_token(&self.kanidm_url, &self.client_id, &device_code).await;

        match status {
            PollStatus::Pending => {}
            PollStatus::SlowDown => {
                // Increase the interval
                if let LoginState::Waiting { poll_interval, .. } = &mut self.state {
                    *poll_interval += 1;
                }
            }
            PollStatus::Success(token) => {
                info!("device flow authentication succeeded");
                // Decode the JWT to get the username (without full validation — the
                // server will validate it properly)
                let username = extract_username_from_jwt(&token.access_token);
                data.user_token = Some(token.access_token);
                data.kanidm_url = Some(self.kanidm_url.clone());
                self.state = LoginState::Authenticated {
                    username: username.unwrap_or_else(|| "unknown".into()),
                };
            }
            PollStatus::Expired => {
                warn!("device code expired");
                self.state =
                    LoginState::Error("Authentication timed out. Please try again.".into());
            }
            PollStatus::AccessDenied => {
                warn!("user denied access");
                self.state = LoginState::Error("Access denied. Please try again.".into());
            }
            PollStatus::Error(e) => {
                error!(error = %e, "token polling error");
                self.state = LoginState::Error(e);
            }
        }
    }
}

fn render_waiting(
    frame: &mut Frame,
    area: Rect,
    flow: &DeviceFlowState,
    qr_lines: &[String],
    elapsed_polls: u64,
) {
    let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spin = spinner[(elapsed_polls as usize) % spinner.len()];

    let display_url = flow
        .verification_uri_complete
        .as_deref()
        .unwrap_or(&flow.verification_uri);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Visit this URL to sign in:",
        Style::default().fg(Color::White),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("    {display_url}"),
        Style::default().fg(Color::Cyan).bold().underlined(),
    )));
    lines.push(Line::from(""));

    // Show user code if the verification_uri_complete isn't available
    if flow.verification_uri_complete.is_none() {
        lines.push(Line::from(Span::styled(
            format!("  Enter code: {}", flow.user_code),
            Style::default().fg(Color::Yellow).bold(),
        )));
        lines.push(Line::from(""));
    }

    // QR code
    lines.push(Line::from(Span::styled(
        "  Scan with your phone:",
        Style::default().fg(ui::MUTED),
    )));
    lines.push(Line::from(""));
    for qr_line in qr_lines {
        lines.push(Line::from(Span::styled(
            format!("    {qr_line}"),
            Style::default().fg(Color::White),
        )));
    }
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled(
        format!("  {spin} Waiting for authentication..."),
        Style::default().fg(Color::Yellow),
    )));

    frame.render_widget(Paragraph::new(lines), area);
}

/// Render a QR code as Unicode half-block characters for terminal display.
fn render_qr_to_lines(data: &str) -> Vec<String> {
    let code = match QrCode::new(data.as_bytes()) {
        Ok(c) => c,
        Err(_) => return vec!["[QR code generation failed]".into()],
    };

    let modules = code.to_colors();
    let width = code.width();
    let mut lines = Vec::new();

    // Process two rows at a time using Unicode half-blocks
    let mut y = 0;
    while y < width {
        let mut line = String::new();
        for x in 0..width {
            let top = modules[y * width + x] == qrcode::Color::Dark;
            let bottom = if y + 1 < width {
                modules[(y + 1) * width + x] == qrcode::Color::Dark
            } else {
                false
            };

            // Use Unicode half-block characters:
            // ▀ = top half (U+2580)
            // ▄ = bottom half (U+2584)
            // █ = full block (U+2588)
            //   = space (both light)
            line.push(match (top, bottom) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            });
        }
        lines.push(line);
        y += 2;
    }

    lines
}

/// Extract the preferred_username or sub from a JWT without validating it.
/// This is for display purposes only — the server validates properly.
fn extract_username_from_jwt(token: &str) -> Option<String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    use base64::Engine;
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[1])
        .ok()?;

    #[derive(serde::Deserialize)]
    struct Claims {
        #[serde(default)]
        preferred_username: Option<String>,
        #[serde(default)]
        sub: Option<String>,
    }

    let claims: Claims = serde_json::from_slice(&payload).ok()?;
    claims.preferred_username.or(claims.sub)
}
