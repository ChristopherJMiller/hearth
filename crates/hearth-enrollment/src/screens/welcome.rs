use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::ui;

pub struct WelcomeScreen;

impl WelcomeScreen {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let center = ui::centered_rect(60, 50, area);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(7), // logo
                Constraint::Length(2), // spacing
                Constraint::Length(1), // welcome text
                Constraint::Length(2), // spacing
                Constraint::Length(1), // instruction
                Constraint::Min(0),
            ])
            .split(center);

        frame.render_widget(ui::logo_text(), layout[0]);

        let welcome = Paragraph::new("Welcome to Hearth Device Enrollment")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::White).bold());
        frame.render_widget(welcome, layout[2]);

        frame.render_widget(
            ui::status_line("Press Enter to begin  |  q to quit"),
            layout[4],
        );
    }

    /// Returns true if the user wants to advance to the next screen.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        matches!(key.code, KeyCode::Enter)
    }
}
