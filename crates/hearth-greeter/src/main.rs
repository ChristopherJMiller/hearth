//! hearth-greeter — GTK4 greetd greeter for Hearth fleet workstations.
//!
//! The greeter is launched by greetd on the login VT. It:
//!
//! 1. Displays a branded fullscreen login window.
//! 2. Authenticates the user via the greetd IPC protocol.
//! 3. Resolves the user's groups via NSS.
//! 4. Asks hearth-agent to prepare the user's environment (home-manager profile, etc.).
//! 5. Shows progress while the agent works.
//! 6. Starts the desktop session via greetd when the environment is ready.
//! 7. Offers a fallback session if preparation fails or times out.

mod agent_client;
mod greetd;
mod nss;
mod ui;

use agent_client::AgentClient;
use glib::ExitCode;
use greetd::{GreetdClient, Response as GreetdResponse};
use gtk4::prelude::*;
use hearth_common::config::GreeterConfig;
use hearth_common::ipc::{AgentEvent, AgentRequest};
use std::time::Duration;
use thiserror::Error;
use tracing::{error, info, warn};
use ui::{UiAction, UiUpdate};

// ---------------------------------------------------------------------------
// Config path
// ---------------------------------------------------------------------------

const CONFIG_PATH: &str = "/etc/hearth/greeter.toml";

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
enum GreeterError {
    #[error("failed to read config: {0}")]
    Config(String),
    #[error("greetd error: {0}")]
    Greetd(#[from] greetd::GreetdError),
    #[error("agent error: {0}")]
    Agent(#[from] agent_client::AgentClientError),
    #[error("NSS error: {0}")]
    Nss(#[from] nss::NssError),
    #[error("{0}")]
    Other(String),
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() -> ExitCode {
    // Initialise logging. When HEARTH_GREETER_LOG_FILE is set, also write
    // logs to that file (useful when greetd doesn't forward child stderr
    // to journal, e.g. in VM integration tests).
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "hearth_greeter=info".into());

    if let Ok(log_path) = std::env::var("HEARTH_GREETER_LOG_FILE") {
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;

        let file = match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            Ok(f) => f,
            Err(e) => {
                eprintln!("hearth-greeter: failed to open log file {log_path}: {e}");
                // Fall back to stderr-only logging.
                tracing_subscriber::fmt().with_env_filter(env_filter).init();
                return ExitCode::FAILURE;
            }
        };

        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(std::sync::Mutex::new(file))
            .with_ansi(false);

        let stderr_layer = tracing_subscriber::fmt::layer();

        tracing_subscriber::registry()
            .with(env_filter)
            .with(stderr_layer)
            .with(file_layer)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    };

    info!("hearth-greeter starting");

    // Load configuration.
    let config = match load_config() {
        Ok(c) => c,
        Err(e) => {
            error!(%e, "failed to load greeter configuration");
            eprintln!("hearth-greeter: fatal: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Headless test mode: skip GTK entirely, drive the login flow from env vars.
    // Set HEARTH_GREETER_TEST_MODE=1, HEARTH_TEST_USER, HEARTH_TEST_PASS.
    if std::env::var("HEARTH_GREETER_TEST_MODE").as_deref() == Ok("1") {
        info!("running in headless test mode");
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                error!(%e, "failed to create tokio runtime for headless mode");
                return ExitCode::FAILURE;
            }
        };
        return rt.block_on(run_headless(config));
    }

    // Build the GTK application.
    let app = gtk4::Application::builder()
        .application_id("com.hearth.greeter")
        .build();

    // We need to share the config with the activate callback.
    let config_for_activate = config.clone();

    app.connect_activate(move |app| {
        let config = config_for_activate.clone();

        // Build the UI; returns channels for communication.
        let (update_tx, action_rx) = ui::build_ui(app, &config.branding);

        // Spawn the async orchestrator on a background tokio runtime.
        // We cannot use #[tokio::main] because GTK owns the main loop.
        let rt = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                error!(%e, "failed to create tokio runtime");
                return;
            }
        };

        // Move the runtime into a thread so its async work runs independently
        // of the GTK main loop. The channels bridge the two worlds.
        std::thread::spawn(move || {
            rt.block_on(orchestrate(config, update_tx, action_rx));
        });
    });

    // If GTK can't initialise at all (no display, etc.) the run() call will
    // print to stderr and return a non-zero exit code, which is the best we
    // can do.
    let exit = app.run_with_args::<&str>(&[]);
    if exit != ExitCode::SUCCESS {
        error!("GTK application exited with failure");
    }
    exit
}

// ---------------------------------------------------------------------------
// Headless test mode
// ---------------------------------------------------------------------------

/// Run the login flow without GTK, reading credentials from environment
/// variables. Used in NixOS VM integration tests.
///
/// Waits up to 120 seconds for `HEARTH_TEST_PASS` to be set (it may be
/// injected after boot once Kanidm bootstrap completes).
async fn run_headless(config: GreeterConfig) -> ExitCode {
    let username = match std::env::var("HEARTH_TEST_USER") {
        Ok(u) => u,
        Err(_) => {
            error!("HEARTH_TEST_USER not set in headless test mode");
            return ExitCode::FAILURE;
        }
    };

    // Read the password from env var or a file. greetd does not pass parent
    // env vars to the greeter process, so in VM tests the password is written
    // to a file by the test script after Kanidm bootstrap completes.
    // We poll the file for up to 120 seconds so the greeter doesn't exit
    // before the test has a chance to write it.
    let password = if let Ok(p) = std::env::var("HEARTH_TEST_PASS") {
        p
    } else {
        let pass_file = std::env::var("HEARTH_TEST_PASS_FILE")
            .unwrap_or_else(|_| "/tmp/hearth-test-pass".to_string());
        info!(%username, %pass_file, "waiting for test password file");
        let mut pass = None;
        for i in 0..120 {
            if let Ok(contents) = tokio::fs::read_to_string(&pass_file).await {
                let trimmed = contents.trim().to_string();
                if !trimmed.is_empty() {
                    pass = Some(trimmed);
                    break;
                }
            }
            if i % 10 == 0 {
                info!(attempt = i, "password file not ready, waiting...");
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        match pass {
            Some(p) => p,
            None => {
                error!("test password not available after 120 seconds");
                return ExitCode::FAILURE;
            }
        }
    };

    info!(%username, "headless login attempt");

    // Create a dummy update channel (log events via tracing instead of UI).
    let (update_tx, update_rx) = async_channel::unbounded::<UiUpdate>();

    // Spawn a task to log UI updates (since there's no GTK to display them).
    tokio::spawn(async move {
        while let Ok(msg) = update_rx.recv().await {
            match msg {
                UiUpdate::AuthSuccess => info!("auth succeeded"),
                UiUpdate::AuthFailed(ref m) => warn!(%m, "auth failed"),
                UiUpdate::PrepProgress {
                    percent,
                    ref message,
                } => {
                    info!(percent, %message, "prep progress")
                }
                UiUpdate::PrepReady => info!("environment ready"),
                UiUpdate::PrepError(ref m) => error!(%m, "prep error"),
            }
        }
    });

    match handle_login(&config, &update_tx, &username, &password).await {
        Ok(()) => {
            info!("headless login succeeded, session started");
            ExitCode::SUCCESS
        }
        Err(e) => {
            error!(%e, "headless login failed");
            ExitCode::FAILURE
        }
    }
}

// ---------------------------------------------------------------------------
// Config loading
// ---------------------------------------------------------------------------

fn load_config() -> Result<GreeterConfig, GreeterError> {
    let contents = std::fs::read_to_string(CONFIG_PATH)
        .map_err(|e| GreeterError::Config(format!("{CONFIG_PATH}: {e}")))?;
    let config: GreeterConfig =
        toml::from_str(&contents).map_err(|e| GreeterError::Config(format!("parse error: {e}")))?;
    info!(
        org = %config.branding.organization_name,
        session = %config.session.command,
        "loaded greeter config"
    );
    Ok(config)
}

// ---------------------------------------------------------------------------
// Async orchestrator
// ---------------------------------------------------------------------------

/// Main orchestration loop. Runs on the tokio runtime (background thread).
/// Communicates with the GTK UI via the provided channels.
async fn orchestrate(
    config: GreeterConfig,
    update_tx: async_channel::Sender<UiUpdate>,
    mut action_rx: tokio::sync::mpsc::Receiver<UiAction>,
) {
    loop {
        // Wait for the user to click login.
        let Some(action) = action_rx.recv().await else {
            info!("UI channel closed, exiting orchestrator");
            return;
        };

        match action {
            UiAction::LoginClicked { username, password } => {
                info!(username = %username, "login attempt");
                match handle_login(&config, &update_tx, &username, &password).await {
                    Ok(()) => {
                        info!("session started successfully, orchestrator done");
                        return;
                    }
                    Err(e) => {
                        warn!(%e, "login flow failed");
                        // The individual handler already sent error updates to the UI.
                        // Continue the loop so the user can try again or use fallback.
                    }
                }
            }
            UiAction::FallbackClicked => {
                info!("user requested fallback session");
                if let Err(e) = start_fallback_session(&config).await {
                    error!(%e, "fallback session failed");
                    let _ = update_tx
                        .send(UiUpdate::PrepError(format!("Fallback session failed: {e}")))
                        .await;
                } else {
                    return;
                }
            }
        }
    }
}

/// Handle a single login attempt end-to-end.
async fn handle_login(
    config: &GreeterConfig,
    update_tx: &async_channel::Sender<UiUpdate>,
    username: &str,
    password: &str,
) -> Result<(), GreeterError> {
    // --- Step 1: Authenticate with greetd ---
    let mut greetd = GreetdClient::connect().await?;

    // Cancel any stale session left over from a previous failed attempt or
    // greeter restart. greetd returns an error if there is no session to
    // cancel, which we intentionally ignore.
    let _ = greetd.cancel_session().await;

    let resp = greetd.create_session(username).await?;
    match handle_auth_flow(&mut greetd, resp, password).await? {
        AuthOutcome::Success => {
            info!("greetd authentication succeeded");
            let _ = update_tx.send(UiUpdate::AuthSuccess).await;
        }
        AuthOutcome::Failed(msg) => {
            warn!(%msg, "authentication failed");
            let _ = update_tx.send(UiUpdate::AuthFailed(msg.clone())).await;
            return Err(GreeterError::Other(msg));
        }
    }

    // --- Step 2: Resolve user groups via NSS ---
    let groups = match nss::get_user_groups(username) {
        Ok(g) => {
            info!(username, ?g, "resolved groups");
            g
        }
        Err(e) => {
            warn!(%e, "failed to resolve groups, proceeding with empty list");
            Vec::new()
        }
    };

    // --- Step 3: Request environment preparation from agent ---
    let session_cmd = match prepare_environment(config, update_tx, username, groups).await {
        Ok(()) => config.session.command.clone(),
        Err(e) => {
            warn!(%e, "environment preparation failed, cancelling greetd session");
            let _ = greetd.cancel_session().await;
            let _ = update_tx
                .send(UiUpdate::PrepError(format!(
                    "Environment preparation failed: {e}. You can use the fallback session."
                )))
                .await;
            return Err(GreeterError::Other(
                "environment preparation failed".to_string(),
            ));
        }
    };

    // --- Step 4: Start the desktop session ---
    let _ = update_tx.send(UiUpdate::PrepReady).await;

    let cmd_parts: Vec<&str> = session_cmd.split_whitespace().collect();
    let resp = greetd.start_session(&cmd_parts).await?;
    match resp {
        GreetdResponse::Success => {
            info!("session started");
            Ok(())
        }
        GreetdResponse::Error { description } => {
            let msg = format!("Failed to start session: {description}");
            let _ = update_tx.send(UiUpdate::PrepError(msg.clone())).await;
            Err(GreeterError::Other(msg))
        }
        other => {
            let msg = format!("Unexpected greetd response starting session: {other:?}");
            let _ = update_tx.send(UiUpdate::PrepError(msg.clone())).await;
            Err(GreeterError::Other(msg))
        }
    }
}

// ---------------------------------------------------------------------------
// greetd auth flow
// ---------------------------------------------------------------------------

enum AuthOutcome {
    Success,
    Failed(String),
}

/// Walk through the greetd authentication message exchange.
///
/// greetd may send multiple `auth_message` prompts (e.g. PAM conversations).
/// We handle:
/// - `secret` -> send the password
/// - `visible` -> send the password (some PAM modules echo)
/// - `info` / `error` -> acknowledge with `None`
async fn handle_auth_flow(
    greetd: &mut GreetdClient,
    initial_response: GreetdResponse,
    password: &str,
) -> Result<AuthOutcome, GreeterError> {
    let mut resp = initial_response;

    loop {
        match resp {
            GreetdResponse::Success => return Ok(AuthOutcome::Success),
            GreetdResponse::Error { description } => {
                return Ok(AuthOutcome::Failed(description));
            }
            GreetdResponse::AuthMessage {
                auth_message_type,
                auth_message: _,
            } => {
                let answer = match auth_message_type.as_str() {
                    "secret" | "visible" => Some(password),
                    // Info/error messages: just acknowledge.
                    _ => None,
                };
                resp = greetd.post_auth_response(answer).await?;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Agent environment preparation
// ---------------------------------------------------------------------------

/// Connect to the agent, request environment preparation, and relay progress
/// back to the UI. Returns `Ok(())` when the agent reports `Ready`.
async fn prepare_environment(
    config: &GreeterConfig,
    update_tx: &async_channel::Sender<UiUpdate>,
    username: &str,
    groups: Vec<String>,
) -> Result<(), GreeterError> {
    let timeout = Duration::from_secs(config.agent.timeout_secs);

    let mut agent = AgentClient::connect(&config.agent.socket_path).await?;

    agent
        .send(&AgentRequest::PrepareUserEnv {
            username: username.to_string(),
            groups,
        })
        .await?;

    let _ = update_tx
        .send(UiUpdate::PrepProgress {
            percent: 0,
            message: "Contacting agent...".to_string(),
        })
        .await;

    // Listen for events with a timeout.
    let result = tokio::time::timeout(timeout, async {
        loop {
            let event = agent.recv().await?;
            match event {
                AgentEvent::Preparing {
                    username: _,
                    message,
                } => {
                    info!(%message, "agent preparing");
                    let _ = update_tx
                        .send(UiUpdate::PrepProgress {
                            percent: 5,
                            message,
                        })
                        .await;
                }
                AgentEvent::Progress {
                    username: _,
                    percent,
                    message,
                } => {
                    info!(percent, %message, "agent progress");
                    let _ = update_tx
                        .send(UiUpdate::PrepProgress { percent, message })
                        .await;
                }
                AgentEvent::Ready { username: _ } => {
                    info!("agent reports environment ready");
                    return Ok::<(), GreeterError>(());
                }
                AgentEvent::Error {
                    username: _,
                    message,
                } => {
                    return Err(GreeterError::Other(message));
                }
                AgentEvent::Pong => {
                    // Unexpected but harmless.
                }
            }
        }
    })
    .await;

    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(_elapsed) => Err(GreeterError::Other(format!(
            "agent did not respond within {} seconds",
            config.agent.timeout_secs
        ))),
    }
}

// ---------------------------------------------------------------------------
// Fallback session
// ---------------------------------------------------------------------------

/// Start the fallback session directly via greetd, skipping agent preparation.
async fn start_fallback_session(config: &GreeterConfig) -> Result<(), GreeterError> {
    let mut greetd = GreetdClient::connect().await?;
    let cmd_parts: Vec<&str> = config.session.fallback_command.split_whitespace().collect();
    let resp = greetd.start_session(&cmd_parts).await?;
    match resp {
        GreetdResponse::Success => {
            info!("fallback session started");
            Ok(())
        }
        GreetdResponse::Error { description } => Err(GreeterError::Other(format!(
            "greetd refused fallback session: {description}"
        ))),
        other => Err(GreeterError::Other(format!(
            "unexpected greetd response: {other:?}"
        ))),
    }
}
