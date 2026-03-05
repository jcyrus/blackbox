use crate::app::App;
use ratatui::{
    Frame,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

impl App {
    pub(crate) fn render_sidebar(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let lines: Vec<Line> = self
            .file_tree
            .nodes
            .iter()
            .enumerate()
            .map(|(idx, node)| {
                let indent = "  ".repeat(node.depth);
                let prefix = if node.is_dir {
                    if self.file_tree.is_expanded(&node.path) {
                        "▾ "
                    } else {
                        "▸ "
                    }
                } else {
                    "  "
                };
                let content = format!("{indent}{prefix}{}", node.name);

                if idx == self.file_tree.selected {
                    Line::from(Span::styled(
                        content,
                        Style::default().fg(Color::Black).bg(Color::Cyan),
                    ))
                } else {
                    Line::from(Span::styled(content, Style::default().fg(Color::Gray)))
                }
            })
            .collect();

        frame.render_widget(
            Paragraph::new(lines).style(Style::default().bg(Color::Rgb(12, 12, 18))),
            area,
        );
    }
}
