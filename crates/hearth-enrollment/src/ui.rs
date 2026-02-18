use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

/// Hearth brand color: ember red
pub const EMBER: Color = Color::Rgb(233, 69, 96); // #e94560
/// Deep navy background accent
#[allow(dead_code)]
pub const NAVY: Color = Color::Rgb(20, 23, 38); // #141726
/// Muted text
pub const MUTED: Color = Color::Rgb(128, 140, 176);

pub fn hearth_block(title: &str) -> Block<'_> {
    Block::default()
        .title(Line::from(title).style(Style::default().fg(EMBER).bold()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(EMBER))
}

pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn logo_text() -> Paragraph<'static> {
    let logo = vec![
        Line::from(Span::styled(
            r"  _   _                _   _     ",
            Style::default().fg(EMBER).bold(),
        )),
        Line::from(Span::styled(
            r" | | | | ___  __ _ _ _| |_| |__  ",
            Style::default().fg(EMBER).bold(),
        )),
        Line::from(Span::styled(
            r" | |_| |/ -_)/ _` | '_|  _| '_ \ ",
            Style::default().fg(EMBER).bold(),
        )),
        Line::from(Span::styled(
            r" |_| |_|\___|\__,_|_|  \__|_| |_|",
            Style::default().fg(EMBER).bold(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Enterprise NixOS Fleet Management",
            Style::default().fg(MUTED).italic(),
        )),
    ];
    Paragraph::new(logo).alignment(Alignment::Center)
}

pub fn status_line(text: &str) -> Paragraph<'_> {
    Paragraph::new(Line::from(Span::styled(text, Style::default().fg(MUTED))))
        .alignment(Alignment::Center)
}
