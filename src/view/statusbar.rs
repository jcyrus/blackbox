use crate::app::{App, FinderMode, same_file_path};
use crate::model::mode::Mode;
use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use ratatui::layout::{Alignment, Constraint, Direction, Layout};

impl App {
    pub(crate) fn render_status_bar(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let (mode_color, mode_bg) = match self.mode {
            Mode::Normal => (Color::Black, Color::Cyan),
            Mode::Insert => (Color::Black, Color::Magenta),
            Mode::Command => (Color::Black, Color::Yellow),
            _ => (Color::White, Color::Rgb(80, 40, 120)),
        };

        let mode_span = Span::styled(
            format!(" {} ", self.mode.label().to_uppercase()),
            Style::default()
                .fg(mode_color)
                .bg(mode_bg)
                .add_modifier(Modifier::BOLD),
        );

        let file_name = self
            .buffer
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "[scratch]".to_string());

        let dirty_marker = if self.buffer.dirty { "  ●" } else { "" };

        let file_info = Span::styled(
            format!("  {file_name}{dirty_marker} "),
            Style::default().fg(Color::Rgb(200, 200, 220)),
        );

        let mut suffix = String::new();
        match self.mode {
            Mode::SidebarCreate => {
                suffix.push_str(&format!(" | new: {}", self.file_tree.create_input))
            }
            Mode::FinderOpen => {
                let label = if self.finder_mode == FinderMode::Files {
                    "find"
                } else {
                    "search"
                };
                suffix.push_str(&format!(" | {label}: {}", self.finder_query));
            }
            Mode::Command => suffix.push_str(&format!(" | :{}", self.command_input)),
            Mode::ConfirmCreate => {
                if let Some(path) = &self.pending_create_path {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "note.md".to_string());
                    suffix.push_str(&format!(" | create {name}? (y/n)"));
                } else {
                    suffix.push_str(" | create note? (y/n)");
                }
            }
            _ => {}
        };

        if self.quit_confirm_armed {
            let pending = self.pending_write_count();
            suffix.push_str(&format!(" | {pending} pending, press q again to quit"));
        }

        let suffix_span = Span::styled(suffix, Style::default().fg(Color::Yellow));

        let left_bar = Line::from(vec![mode_span, file_info, suffix_span]);

        let right_spans = vec![
            Span::styled(
                " MD ",
                Style::default()
                    .bg(Color::Rgb(30, 30, 45))
                    .fg(Color::Rgb(150, 150, 170)),
            ),
            Span::styled(
                format!("  {} w ", self.buffer.word_count()),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                format!(
                    "  {}:{} ",
                    self.buffer.cursor.row + 1,
                    self.buffer.cursor.col + 1
                ),
                Style::default()
                    .bg(mode_bg)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
        ];

        let right_bar = Line::from(right_spans).alignment(Alignment::Right);

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(30)])
            .split(area);

        frame.render_widget(
            Paragraph::new(left_bar).style(Style::default().bg(Color::Rgb(15, 15, 24))),
            chunks[0],
        );
        frame.render_widget(
            Paragraph::new(right_bar).style(Style::default().bg(Color::Rgb(15, 15, 24))),
            chunks[1],
        );
    }
    pub(crate) fn render_tab_bar(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let active_path = self.buffer.path.as_ref();
        let mut spans = Vec::new();

        for tab_path in &self.open_tabs {
            let name = tab_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "[note]".to_string());

            let is_active = active_path.is_some_and(|p| same_file_path(p, tab_path));
            let mut label = format!(" {name} ");
            if is_active && self.buffer.dirty {
                label = format!(" {name} ● ");
            }

            let style = if is_active {
                Style::default()
                    .bg(Color::Rgb(30, 30, 45))
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().bg(Color::Rgb(18, 18, 28)).fg(Color::Gray)
            };

            spans.push(Span::styled(label, style));
        }

        spans.push(Span::styled(
            "  [Space] Leader ",
            Style::default()
                .bg(Color::Rgb(20, 20, 30))
                .fg(Color::DarkGray),
        ));

        let line = Line::from(spans);

        frame.render_widget(
            Paragraph::new(line).style(Style::default().bg(Color::Rgb(20, 20, 30))),
            area,
        );
    }
}
