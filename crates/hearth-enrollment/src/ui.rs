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

/// Wrap a long error message into multiple indented `Line`s that fit within
/// `max_width` characters.  Splits on word boundaries where possible, with a
/// hard cap of `max_width` per line.  Returns at most 8 lines; the last line
/// is truncated with "…" if the message is longer.
pub fn textwrap_lines(msg: &str, max_width: usize, color: Color) -> Vec<Line<'static>> {
    const MAX_LINES: usize = 8;
    let width = max_width.max(20);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut remaining = msg;

    while !remaining.is_empty() {
        if lines.len() >= MAX_LINES {
            // Replace last line with truncated version.
            if let Some(last) = lines.last_mut() {
                let mut trunc: String = last.to_string();
                if trunc.len() > 3 {
                    trunc.truncate(trunc.len() - 1);
                }
                trunc.push('…');
                *last = Line::from(Span::styled(trunc, Style::default().fg(color)));
            }
            break;
        }

        if remaining.len() <= width {
            lines.push(Line::from(Span::styled(
                format!("  {remaining}"),
                Style::default().fg(color),
            )));
            break;
        }

        // Find a word boundary to break at.
        let break_at = remaining[..width].rfind(' ').unwrap_or(width);
        let (chunk, rest) = remaining.split_at(break_at);
        lines.push(Line::from(Span::styled(
            format!("  {chunk}"),
            Style::default().fg(color),
        )));
        remaining = rest.trim_start();
    }
    lines
}
