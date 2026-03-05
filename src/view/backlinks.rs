use crate::app::App;
use ratatui::{
    Frame,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

impl App {
    pub(crate) fn render_backlinks_panel(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let lines: Vec<Line> = if self.backlinks.is_empty() {
            vec![Line::from(Span::styled(
                "No backlinks",
                Style::default().fg(Color::DarkGray),
            ))]
        } else {
            self.backlinks
                .iter()
                .enumerate()
                .map(|(idx, entry)| {
                    let file = entry
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "[note]".to_string());
                    let mut preview = entry.preview.clone();
                    if preview.len() > 42 {
                        preview.truncate(42);
                        preview.push('…');
                    }

                    let label = format!("{file}:{}  {preview}", entry.line);
                    if idx == self.backlinks_selected {
                        Line::from(Span::styled(
                            label,
                            Style::default().fg(Color::Black).bg(Color::Cyan),
                        ))
                    } else {
                        Line::from(Span::styled(label, Style::default().fg(Color::Gray)))
                    }
                })
                .collect()
        };

        let panel = Paragraph::new(lines).block(
            Block::default()
                .title(" Backlinks ")
                .borders(Borders::LEFT)
                .style(Style::default().bg(Color::Rgb(12, 12, 18))),
        );
        frame.render_widget(panel, area);
    }
}
