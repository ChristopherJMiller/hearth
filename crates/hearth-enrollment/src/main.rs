mod app;
mod hw;
mod oauth;
mod screens;
mod ui;

use std::io;
use std::time::Duration;

use app::{App, AppResult};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::prelude::*;
use tracing::info;

#[tokio::main]
async fn main() -> AppResult<()> {
    // Write logs to a file so they don't corrupt the TUI.
    let log_file =
        std::fs::File::create("/tmp/hearth-enrollment.log").expect("failed to create log file");

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hearth_enrollment=info".into()),
        )
        .with_writer(log_file)
        .with_ansi(false)
        .init();

    info!("hearth-enrollment starting");

    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let result = run_app(&mut terminal, &mut app).await;

    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> AppResult<()> {
    loop {
        terminal.draw(|frame| app.render(frame))?;

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            if key.code == KeyCode::Char('q') && app.can_quit() {
                return Ok(());
            }
            app.handle_key(key).await;
        }

        app.tick().await;

        // The login screen may request a kiosk browser launch. This requires
        // suspending the TUI so cage can take over the VT.
        if let Some((auth_url, callback_rx)) = app.take_browser_request() {
            launch_kiosk_browser(terminal, app, &auth_url, callback_rx).await?;
        }

        if app.should_exit() {
            return Ok(());
        }
    }
}

/// Suspend the TUI, launch cage + Firefox for OAuth authentication, then restore.
///
/// cage is a minimal Wayland kiosk compositor that needs DRM/VT access. The TUI
/// must release raw mode and the alternate screen so cage can take over the
/// display. The browser is auto-closed as soon as the OAuth callback arrives
/// (signaled via `callback_rx`) — the user doesn't have to manually close
/// the window. If the user closes Firefox first, `child.wait()` wins the
/// select and we restore the TUI anyway.
async fn launch_kiosk_browser(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    auth_url: &str,
    callback_rx: tokio::sync::oneshot::Receiver<()>,
) -> AppResult<()> {
    // Suspend TUI — release the VT so cage can take over
    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    info!("suspended TUI for kiosk browser");

    // Create a temporary Firefox profile with permissive settings for the
    // enrollment kiosk. Kanidm uses self-signed certs and session cookies for
    // multi-step auth — a fresh Firefox profile may block these.
    let profile_dir = std::env::temp_dir().join("hearth-firefox-profile");
    std::fs::create_dir_all(&profile_dir).ok();
    let user_js = profile_dir.join("user.js");
    std::fs::write(
        &user_js,
        r#"// Hearth enrollment kiosk — auto-generated
user_pref("network.cookie.cookieBehavior", 0);
user_pref("network.cookie.lifetimePolicy", 0);
user_pref("privacy.trackingprotection.enabled", false);
user_pref("browser.shell.checkDefaultBrowser", false);
user_pref("datareporting.policy.dataSubmissionEnabled", false);
user_pref("toolkit.telemetry.reportingpolicy.firstRun", false);
user_pref("browser.aboutwelcome.enabled", false);
user_pref("browser.startup.homepage_override.mstone", "ignore");
user_pref("security.enterprise_roots.enabled", true);
"#,
    )
    .ok();

    // Launch cage + Firefox with pixman (software) rendering. The enrollment
    // kiosk only displays a login form briefly, so software rendering is fine
    // even on real hardware. This avoids DRM/Vulkan mismatch issues on VMs and
    // works reliably everywhere.
    let mut cmd = tokio::process::Command::new("cage");
    cmd.env("WLR_RENDERER", "pixman");
    cmd.env("WLR_NO_HARDWARE_CURSORS", "1");
    // Firefox needs explicit Wayland opt-in — without this it tries X11
    // which doesn't exist inside cage, resulting in a black screen.
    cmd.env("MOZ_ENABLE_WAYLAND", "1");
    cmd.arg("--")
        .arg("firefox")
        .arg("--kiosk")
        .arg("--no-remote")
        .arg("--profile")
        .arg(&profile_dir)
        .arg(auth_url);

    match cmd.spawn() {
        Ok(mut child) => {
            // Wait until EITHER:
            //   (a) the OAuth callback fires — then we kill cage (and
            //       firefox with it) so the user doesn't have to manually
            //       close the kiosk; the token exchange continues in the
            //       background and is picked up on the next TUI tick, OR
            //   (b) Firefox exits on its own (user closed it / crashed) —
            //       whichever happens first.
            tokio::select! {
                _ = callback_rx => {
                    info!("OAuth callback received, closing kiosk browser");
                    // Graceful kill: cage cleans up firefox as its child.
                    if let Err(e) = child.kill().await {
                        tracing::warn!(error = %e, "failed to kill cage cleanly");
                    }
                    let _ = child.wait().await;
                }
                wait_result = child.wait() => {
                    match wait_result {
                        Ok(status) => info!(?status, "kiosk browser exited"),
                        Err(e) => tracing::error!(error = %e, "failed to wait on kiosk browser"),
                    }
                }
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to launch kiosk browser");
            app.notify_browser_failed(format!(
                "Failed to launch browser: {e}. Press Enter to retry."
            ));
        }
    }

    // Brief pause to let the token exchange complete if it's in-flight
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Restore TUI
    terminal::enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    terminal.clear()?;

    info!("restored TUI after kiosk browser");

    Ok(())
}
