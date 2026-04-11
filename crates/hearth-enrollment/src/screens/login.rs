//! Login screen: interactive username/password form against Kanidm.
//!
//! Authenticates directly via Kanidm's REST API (3-step auth flow).
//! Falls back from env-var token injection → env-var credentials →
//! interactive TUI form.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use tracing::{error, info};

use crate::app::EnrollmentData;
use crate::oauth;
use crate::ui;

#[derive(Clone, Copy, PartialEq)]
enum Field {
    Username,
    Password,
}

enum LoginState {
    /// Interactive credential input form.
    InteractiveLogin {
        username: String,
        password: String,
        focused: Field,
        error: Option<String>,
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
            Self::InteractiveLogin { username, .. } => f
                .debug_struct("InteractiveLogin")
                .field("username", username)
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
}

impl LoginScreen {
    pub fn new() -> Self {
        let kanidm_url = std::env::var("HEARTH_KANIDM_URL")
            .unwrap_or_else(|_| "https://kanidm.hearth.local:8443".into());
        let client_id =
            std::env::var("HEARTH_KANIDM_CLIENT_ID").unwrap_or_else(|_| "hearth-enrollment".into());

        // Auth bypass: token injection takes priority, then env-var credentials,
        // then interactive form.
        let state = if let Ok(token) = std::env::var("HEARTH_AUTH_TOKEN") {
            if !token.is_empty() {
                info!("HEARTH_AUTH_TOKEN set, will bypass login");
                LoginState::TokenInjected { token }
            } else {
                LoginState::InteractiveLogin {
                    username: String::new(),
                    password: String::new(),
                    focused: Field::Username,
                    error: None,
                }
            }
        } else if let (Ok(username), Ok(password)) = (
            std::env::var("HEARTH_AUTH_USERNAME"),
            std::env::var("HEARTH_AUTH_PASSWORD"),
        ) {
            if !username.is_empty() && !password.is_empty() {
                info!(username = %username, "HEARTH_AUTH_USERNAME/PASSWORD set, will use direct credential auth");
                LoginState::DirectAuth { username, password }
            } else {
                LoginState::InteractiveLogin {
                    username: String::new(),
                    password: String::new(),
                    focused: Field::Username,
                    error: None,
                }
            }
        } else {
            LoginState::InteractiveLogin {
                username: String::new(),
                password: String::new(),
                focused: Field::Username,
                error: None,
            }
        };

        Self {
            state,
            kanidm_url,
            client_id,
        }
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let center = ui::centered_rect(80, 80, area);
        let block = ui::hearth_block(" Sign In ");

        let inner = block.inner(center);
        frame.render_widget(block, center);

        match &self.state {
            LoginState::InteractiveLogin {
                username,
                password,
                focused,
                error,
            } => {
                let username_style = if *focused == Field::Username {
                    Style::default().fg(ui::EMBER).bold()
                } else {
                    Style::default().fg(Color::White)
                };
                let password_style = if *focused == Field::Password {
                    Style::default().fg(ui::EMBER).bold()
                } else {
                    Style::default().fg(Color::White)
                };
                let cursor_on = "▎";
                let cursor_off = " ";
                let u_cursor = if *focused == Field::Username { cursor_on } else { cursor_off };
                let p_cursor = if *focused == Field::Password { cursor_on } else { cursor_off };
                let masked: String = "•".repeat(password.len());

                let mut lines = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Sign in with your Kanidm credentials",
                        Style::default().fg(ui::MUTED),
                    )),
                    Line::from(""),
                    Line::from(""),
                    Line::from(Span::styled("  Username", username_style)),
                    Line::from(Span::styled(
                        format!("  {u_cursor} {username}"),
                        username_style,
                    )),
                    Line::from(""),
                    Line::from(Span::styled("  Password", password_style)),
                    Line::from(Span::styled(
                        format!("  {p_cursor} {masked}"),
                        password_style,
                    )),
                    Line::from(""),
                ];

                if let Some(err) = error {
                    lines.push(Line::from(Span::styled(
                        format!("  {err}"),
                        Style::default().fg(Color::Red),
                    )));
                    lines.push(Line::from(""));
                }

                lines.push(Line::from(Span::styled(
                    "  Tab/↑↓: switch field  Enter: sign in",
                    Style::default().fg(ui::MUTED),
                )));

                frame.render_widget(Paragraph::new(lines), inner);
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
                ];
                let max_width = inner.width.saturating_sub(4) as usize;
                let err_lines = ui::textwrap_lines(err, max_width, Color::Red);
                let footer = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Press Enter to retry",
                        Style::default().fg(ui::MUTED),
                    )),
                ];
                let all: Vec<Line> = items
                    .into_iter()
                    .chain(err_lines)
                    .chain(footer)
                    .collect();
                frame.render_widget(Paragraph::new(all), inner);
            }
        }
    }

    /// Returns true when login is complete and we should advance.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match &mut self.state {
            LoginState::InteractiveLogin {
                username,
                password,
                focused,
                ..
            } => {
                match key.code {
                    KeyCode::Tab | KeyCode::BackTab | KeyCode::Up | KeyCode::Down => {
                        *focused = match focused {
                            Field::Username => Field::Password,
                            Field::Password => Field::Username,
                        };
                    }
                    KeyCode::Enter => {
                        if *focused == Field::Username {
                            // Enter on username moves to password
                            *focused = Field::Password;
                        } else if !username.is_empty() && !password.is_empty() {
                            let u = username.clone();
                            let p = password.clone();
                            self.state = LoginState::DirectAuth {
                                username: u,
                                password: p,
                            };
                        }
                    }
                    KeyCode::Backspace => match focused {
                        Field::Username => {
                            username.pop();
                        }
                        Field::Password => {
                            password.pop();
                        }
                    },
                    KeyCode::Char(c) => match focused {
                        Field::Username => username.push(c),
                        Field::Password => password.push(c),
                    },
                    _ => {}
                }
                false
            }
            LoginState::Authenticated { .. } => {
                matches!(key.code, KeyCode::Enter)
            }
            LoginState::Error(_) => {
                if matches!(key.code, KeyCode::Enter) {
                    self.state = LoginState::InteractiveLogin {
                        username: String::new(),
                        password: String::new(),
                        focused: Field::Username,
                        error: None,
                    };
                }
                false
            }
            _ => false,
        }
    }

    /// Called on each tick. Checks for token injection / credential auth completion.
    pub async fn tick(&mut self, data: &mut EnrollmentData) -> bool {
        match &self.state {
            LoginState::TokenInjected { .. } => {
                let token = match std::mem::replace(
                    &mut self.state,
                    LoginState::InteractiveLogin {
                        username: String::new(),
                        password: String::new(),
                        focused: Field::Username,
                        error: None,
                    },
                ) {
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
                let (username, password) = match std::mem::replace(
                    &mut self.state,
                    LoginState::InteractiveLogin {
                        username: String::new(),
                        password: String::new(),
                        focused: Field::Username,
                        error: None,
                    },
                ) {
                    LoginState::DirectAuth { username, password } => (username, password),
                    _ => unreachable!(),
                };
                let kanidm_url = self.kanidm_url.clone();
                let client_id = self.client_id.clone();
                info!(username = %username, "starting direct credential auth against Kanidm");
                match oauth::authenticate_with_credentials(&kanidm_url, &client_id, &username, &password).await
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
            LoginState::InteractiveLogin { .. } => false,
            LoginState::Authenticated { .. } => true,
            LoginState::Error(_) => false,
        }
    }
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
