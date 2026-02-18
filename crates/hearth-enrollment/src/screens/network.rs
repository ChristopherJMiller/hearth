use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::app::EnrollmentData;
use crate::ui;

#[derive(Debug, PartialEq)]
enum NetStatus {
    Checking,
    Connected,
    NoNetwork,
}

pub struct NetworkScreen {
    status: NetStatus,
}

impl NetworkScreen {
    pub fn new() -> Self {
        Self {
            status: NetStatus::Checking,
        }
    }

    pub fn check(&mut self, data: &mut EnrollmentData) {
        // Simple connectivity check: see if we have a global IP
        if data.ip_address == "no address" || data.ip_address.is_empty() {
            self.status = NetStatus::NoNetwork;
        } else {
            self.status = NetStatus::Connected;
        }
    }

    pub fn render(&self, frame: &mut Frame, data: &EnrollmentData) {
        let area = frame.area();
        let center = ui::centered_rect(60, 40, area);
        let block = ui::hearth_block(" Network Status ");

        let inner = block.inner(center);
        frame.render_widget(block, center);

        let (status_text, status_color) = match self.status {
            NetStatus::Checking => ("Checking...", Color::Yellow),
            NetStatus::Connected => ("Connected", Color::Green),
            NetStatus::NoNetwork => ("No Network", Color::Red),
        };

        let items = vec![
            Line::from(vec![
                Span::styled("  Status:  ", Style::default().fg(ui::MUTED)),
                Span::styled(status_text, Style::default().fg(status_color).bold()),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  IP:      ", Style::default().fg(ui::MUTED)),
                Span::styled(&data.ip_address, Style::default().fg(Color::White)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                match self.status {
                    NetStatus::Connected => "  Press Enter to continue  |  q to quit",
                    NetStatus::NoNetwork => "  Please configure network and restart enrollment",
                    NetStatus::Checking => "  Please wait...",
                },
                Style::default().fg(ui::MUTED),
            )),
        ];

        frame.render_widget(Paragraph::new(items), inner);
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        self.status == NetStatus::Connected && matches!(key.code, KeyCode::Enter)
    }
}
