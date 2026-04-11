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

        if app.should_exit() {
            return Ok(());
        }
    }
}
