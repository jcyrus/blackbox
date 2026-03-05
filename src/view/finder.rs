use crate::app::{App, FinderMode, centered_rect};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

impl App {
    pub(crate) fn render_finder_overlay(&self, frame: &mut Frame) {
        let area = centered_rect(70, 60, frame.area());
        frame.render_widget(Clear, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(area);

        let input = Paragraph::new(self.finder_query.clone()).block(
            Block::default()
                .title(if self.finder_mode == FinderMode::Files {
                    " Finder (Files) "
                } else {
                    " Search (Content) "
                })
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Rgb(15, 15, 24))),
        );
        frame.render_widget(input, chunks[0]);

        let results: Vec<Line> = if self.finder_results.is_empty() {
            vec![Line::from(Span::styled(
                "No matches",
                Style::default().fg(Color::DarkGray),
            ))]
        } else {
            self.finder_results
                .iter()
                .enumerate()
                .map(|(idx, item)| {
                    let label = item.preview.clone();
                    if idx == self.finder_selected {
                        Line::from(Span::styled(
                            format!("> {label}"),
                            Style::default().fg(Color::Black).bg(Color::Cyan),
                        ))
                    } else {
                        Line::from(Span::styled(
                            format!("  {label}"),
                            Style::default().fg(Color::Gray),
                        ))
                    }
                })
                .collect()
        };

        let result_block = Paragraph::new(results).block(
            Block::default()
                .borders(Borders::LEFT | Borders::RIGHT)
                .style(Style::default().bg(Color::Rgb(10, 10, 18))),
        );
        frame.render_widget(result_block, chunks[1]);

        let footer = Paragraph::new(" Enter: open  Esc: close  j/k: move ").block(
            Block::default().borders(Borders::ALL).style(
                Style::default()
                    .bg(Color::Rgb(15, 15, 24))
                    .fg(Color::DarkGray),
            ),
        );
        frame.render_widget(footer, chunks[2]);

        let cursor_x = chunks[0].x + 1 + self.finder_query.len() as u16;
        let cursor_y = chunks[0].y + 1;
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}
