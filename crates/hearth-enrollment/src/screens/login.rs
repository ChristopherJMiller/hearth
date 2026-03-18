//! Login screen: OAuth2 Authorization Code + PKCE with kiosk browser.
//!
//! Launches Firefox in kiosk mode inside `cage` (Wayland kiosk compositor)
//! for the user to authenticate via Kanidm. A local HTTP callback server
//! receives the authorization code redirect.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use tokio::sync::oneshot;
use tracing::{error, info};

use crate::app::EnrollmentData;
use crate::oauth::{self, AuthToken};
use crate::ui;

enum LoginState {
    /// Waiting for the flow to start (initial state).
    Ready,
    /// Browser launched, waiting for user to authenticate.
    WaitingForAuth {
        token_rx: Option<oneshot::Receiver<Result<AuthToken, String>>>,
        elapsed_ticks: u64,
    },
    /// Direct credential auth pending (no browser). Transitions to
    /// `Authenticated` or `Error` on the first tick — never persists across ticks.
    DirectAuth { username: String, password: String },
    /// Token was injected via env var, skip login entirely.
    TokenInjected { token: String },
    /// Authentication succeeded, user token acquired.
    Authenticated { username: String },
    /// Something went wrong.
    Error(String),
}

impl std::fmt::Debug for LoginState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ready => write!(f, "Ready"),
            Self::WaitingForAuth { elapsed_ticks, .. } => f
                .debug_struct("WaitingForAuth")
                .field("elapsed_ticks", elapsed_ticks)
                .finish_non_exhaustive(),
            Self::DirectAuth { username, .. } => f
                .debug_struct("DirectAuth")
                .field("username", username)
                .finish_non_exhaustive(),
            Self::TokenInjected { .. } => write!(f, "TokenInjected"),
            Self::Authenticated { username } => f
                .debug_struct("Authenticated")
                .field("username", username)
                .finish(),
            Self::Error(e) => f.debug_tuple("Error").field(e).finish(),
        }
    }
}

pub struct LoginScreen {
    state: LoginState,
    /// Kanidm URL for the auth flow.
    kanidm_url: String,
    /// OAuth2 client ID for enrollment.
    client_id: String,
    /// Whether the initial flow has been kicked off.
    started: bool,
    /// Auth URL that needs to be opened in a kiosk browser.
    /// Set by `start_flow`, consumed by the main loop via `take_browser_request`.
    pending_browser_url: Option<String>,
}

impl LoginScreen {
    pub fn new() -> Self {
        let kanidm_url = std::env::var("HEARTH_KANIDM_URL")
            .unwrap_or_else(|_| "https://kanidm.hearth.local:8443".into());
        let client_id =
            std::env::var("HEARTH_KANIDM_CLIENT_ID").unwrap_or_else(|_| "hearth-enrollment".into());

        // Auth bypass: token injection takes priority, then credential auth, then browser flow.
        let state = if let Ok(token) = std::env::var("HEARTH_AUTH_TOKEN") {
            if !token.is_empty() {
                info!("HEARTH_AUTH_TOKEN set, will bypass browser login");
                LoginState::TokenInjected { token }
            } else {
                LoginState::Ready
            }
        } else if let (Ok(username), Ok(password)) = (
            std::env::var("HEARTH_AUTH_USERNAME"),
            std::env::var("HEARTH_AUTH_PASSWORD"),
        ) {
            if !username.is_empty() && !password.is_empty() {
                info!(username = %username, "HEARTH_AUTH_USERNAME/PASSWORD set, will use direct credential auth");
                LoginState::DirectAuth { username, password }
            } else {
                LoginState::Ready
            }
        } else {
            LoginState::Ready
        };

        Self {
            state,
            kanidm_url,
            client_id,
            started: false,
            pending_browser_url: None,
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
                        "  Preparing authentication...",
                        Style::default().fg(Color::Yellow),
                    )),
                ];
                frame.render_widget(Paragraph::new(items), inner);
            }
            LoginState::TokenInjected { .. } => {
                let items = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Using injected auth token...",
                        Style::default().fg(Color::Yellow),
                    )),
                ];
                frame.render_widget(Paragraph::new(items), inner);
            }
            LoginState::DirectAuth { username, .. } => {
                let items = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("  Authenticating as {username}..."),
                        Style::default().fg(Color::Yellow),
                    )),
                ];
                frame.render_widget(Paragraph::new(items), inner);
            }
            LoginState::WaitingForAuth { elapsed_ticks, .. } => {
                render_waiting(frame, inner, *elapsed_ticks);
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
        match &mut self.state {
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
            LoginState::WaitingForAuth { .. } => {
                if matches!(key.code, KeyCode::Esc) {
                    self.state =
                        LoginState::Error("Authentication cancelled. Press Enter to retry.".into());
                }
                false
            }
            _ => false,
        }
    }

    /// Take the pending browser URL (if any) for the main loop to launch.
    pub fn take_browser_request(&mut self) -> Option<String> {
        self.pending_browser_url.take()
    }

    /// Notify the login screen that the browser failed to launch.
    pub fn notify_browser_failed(&mut self, err: String) {
        self.state = LoginState::Error(err);
    }

    /// Called on each tick. Starts the auth flow or checks for completion.
    pub async fn tick(&mut self, data: &mut EnrollmentData) -> bool {
        match &self.state {
            LoginState::TokenInjected { .. } => {
                // Direct token injection — use token captured at construction time.
                let token = match std::mem::replace(&mut self.state, LoginState::Ready) {
                    LoginState::TokenInjected { token } => token,
                    _ => unreachable!(),
                };
                let username =
                    extract_username_from_jwt(&token).unwrap_or_else(|| "token-auth".into());
                info!(username = %username, "using injected auth token");
                data.user_token = Some(token);
                data.kanidm_url = Some(self.kanidm_url.clone());
                self.state = LoginState::Authenticated { username };
                true
            }
            LoginState::DirectAuth { .. } => {
                // Extract credentials by replacing state, then authenticate inline.
                // This always transitions to Authenticated or Error before returning.
                let (username, password) =
                    match std::mem::replace(&mut self.state, LoginState::Ready) {
                        LoginState::DirectAuth { username, password } => (username, password),
                        _ => unreachable!(),
                    };
                let kanidm_url = self.kanidm_url.clone();
                info!(username = %username, "starting direct credential auth against Kanidm");
                match oauth::authenticate_with_credentials(&kanidm_url, &username, &password).await
                {
                    Ok(token) => {
                        let display_name = extract_username_from_jwt(&token.access_token)
                            .unwrap_or_else(|| username.clone());
                        info!(username = %display_name, "direct credential auth succeeded");
                        data.user_token = Some(token.access_token);
                        data.kanidm_url = Some(kanidm_url);
                        self.state = LoginState::Authenticated {
                            username: display_name,
                        };
                        true
                    }
                    Err(e) => {
                        error!(error = %e, "direct credential auth failed");
                        self.state = LoginState::Error(e);
                        false
                    }
                }
            }
            LoginState::Ready => {
                if !self.started {
                    self.started = true;
                    self.start_flow().await;
                }
                false
            }
            LoginState::WaitingForAuth { .. } => {
                self.check_auth(data).await;
                matches!(self.state, LoginState::Authenticated { .. })
            }
            LoginState::Authenticated { .. } => true,
            LoginState::Error(_) => false,
        }
    }

    async fn start_flow(&mut self) {
        match oauth::start_auth_code_flow(&self.kanidm_url, &self.client_id).await {
            Ok(handle) => {
                // Store the auth URL for the main loop to open in a kiosk browser.
                // The main loop handles terminal suspension and cage launch.
                self.pending_browser_url = Some(handle.auth_url);
                info!("OAuth flow started, browser launch requested");
                self.state = LoginState::WaitingForAuth {
                    token_rx: Some(handle.token_rx),
                    elapsed_ticks: 0,
                };
            }
            Err(e) => {
                error!(error = %e, "failed to start auth flow");
                self.state = LoginState::Error(e);
            }
        }
    }

    async fn check_auth(&mut self, data: &mut EnrollmentData) {
        let LoginState::WaitingForAuth {
            token_rx,
            elapsed_ticks,
        } = &mut self.state
        else {
            return;
        };

        *elapsed_ticks += 1;

        // Check if the token channel has a result
        if let Some(rx) = token_rx.as_mut() {
            match rx.try_recv() {
                Ok(Ok(token)) => {
                    info!("authentication succeeded");
                    let username = extract_username_from_jwt(&token.access_token);
                    data.user_token = Some(token.access_token);
                    data.kanidm_url = Some(self.kanidm_url.clone());
                    self.state = LoginState::Authenticated {
                        username: username.unwrap_or_else(|| "unknown".into()),
                    };
                }
                Ok(Err(e)) => {
                    error!(error = %e, "authentication failed");
                    self.state = LoginState::Error(e);
                }
                Err(oneshot::error::TryRecvError::Empty) => {
                    // Still waiting
                }
                Err(oneshot::error::TryRecvError::Closed) => {
                    self.state =
                        LoginState::Error("Authentication callback failed unexpectedly.".into());
                }
            }
        }
    }
}

fn render_waiting(frame: &mut Frame, area: Rect, elapsed_ticks: u64) {
    let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spin = spinner[(elapsed_ticks as usize) % spinner.len()];

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  A browser window has opened for authentication.",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Complete sign-in in the browser to continue.",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            format!("  {spin} Waiting for authentication..."),
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Press Esc to cancel",
            Style::default().fg(ui::MUTED),
        )),
    ];

    frame.render_widget(Paragraph::new(lines), area);
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
        name: Option<String>,
        #[serde(default)]
        spn: Option<String>,
        #[serde(default)]
        sub: Option<String>,
    }

    let claims: Claims = serde_json::from_slice(&payload).ok()?;
    claims
        .preferred_username
        .or(claims.name)
        .or(claims.spn)
        .or(claims.sub)
}
