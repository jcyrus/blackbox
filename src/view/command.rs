use crate::app::{App, centered_rect};
use ratatui::{
    Frame,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

impl App {
    pub(crate) fn render_command_overlay(&self, frame: &mut Frame) {
        let area = centered_rect(70, 20, frame.area());
        frame.render_widget(Clear, area);

        let prompt = Paragraph::new(format!(":{}", self.command_input)).block(
            Block::default()
                .title(" Command ")
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Rgb(15, 15, 24))),
        );
        frame.render_widget(prompt, area);

        let cursor_x = area.x + 2 + self.command_input.len() as u16;
        let cursor_y = area.y + 1;
        frame.set_cursor_position((cursor_x, cursor_y));
    }

    pub(crate) fn render_which_key(&self, frame: &mut Frame) {
        let area = centered_rect(40, 40, frame.area());

        let lines = vec![
            Line::from(Span::styled(
                "  f  Find files",
                Style::default().fg(Color::Cyan),
            )),
            Line::from(Span::styled(
                "  g  Grep content",
                Style::default().fg(Color::Cyan),
            )),
            Line::from(Span::styled(
                "  e  Explorer / Sidebar",
                Style::default().fg(Color::Cyan),
            )),
            Line::from(Span::styled(
                "  b  Backlinks",
                Style::default().fg(Color::Cyan),
            )),
            Line::from(Span::styled(
                "  n  New note",
                Style::default().fg(Color::Cyan),
            )),
            Line::from(Span::styled(
                "  p  Plugins",
                Style::default().fg(Color::Cyan),
            )),
            Line::from(Span::styled("  h  Help", Style::default().fg(Color::Cyan))),
            Line::from(Span::raw("")),
            Line::from(Span::styled(
                "  Press key or Esc to cancel",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        frame.render_widget(Clear, area);

        let popup = Paragraph::new(lines).block(
            Block::default()
                .title(" Leader ")
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Rgb(15, 15, 24))),
        );
        frame.render_widget(popup, area);
    }
}
