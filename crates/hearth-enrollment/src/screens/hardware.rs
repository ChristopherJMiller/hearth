use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::app::EnrollmentData;
use crate::hw;
use crate::ui;

pub struct HardwareScreen {
    detected: bool,
}

impl HardwareScreen {
    pub fn new() -> Self {
        Self { detected: false }
    }

    pub fn detect(&mut self, data: &mut EnrollmentData) {
        hw::detect_all(data);
        self.detected = true;
    }

    pub fn render(&self, frame: &mut Frame, data: &EnrollmentData) {
        let area = frame.area();
        let center = ui::centered_rect(70, 60, area);
        let block = ui::hearth_block(" Hardware Detection ");

        let inner = block.inner(center);
        frame.render_widget(block, center);

        let items = vec![
            Line::from(vec![
                Span::styled("  Hostname:  ", Style::default().fg(ui::MUTED)),
                Span::styled(&data.hostname, Style::default().fg(Color::White).bold()),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  CPU:       ", Style::default().fg(ui::MUTED)),
                Span::styled(&data.cpu, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("  RAM:       ", Style::default().fg(ui::MUTED)),
                Span::styled(&data.ram, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("  Disk:      ", Style::default().fg(ui::MUTED)),
                Span::styled(&data.disk, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("  NIC:       ", Style::default().fg(ui::MUTED)),
                Span::styled(&data.nic, Style::default().fg(Color::White)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                if self.detected {
                    "  Press Enter to continue  |  q to quit"
                } else {
                    "  Detecting hardware..."
                },
                Style::default().fg(ui::MUTED),
            )),
        ];

        let paragraph = Paragraph::new(items);
        frame.render_widget(paragraph, inner);
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        self.detected && matches!(key.code, KeyCode::Enter)
    }
}
