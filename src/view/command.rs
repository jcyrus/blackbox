use crate::app::{App, centered_rect};
use ratatui::{
    Frame,
    style::{Color, Style},
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
}
