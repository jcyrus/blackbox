use crate::app::{App, FinderMode, same_file_path};
use crate::model::mode::Mode;
use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

impl App {
    pub(crate) fn render_status_bar(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let mode_style = match self.mode {
            Mode::Normal => Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
            Mode::Insert => Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            _ => Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        };

        let mode_span = Span::styled(format!(" {} ", self.mode.label()), mode_style);

        let file_name = self
            .buffer
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "[scratch]".to_string());

        let dirty_marker = if self.buffer.dirty { " [+]" } else { "" };

        let mut suffix = match self.mode {
            Mode::SidebarCreate => format!(" | new: {}", self.file_tree.create_input),
            Mode::FinderOpen => {
                let label = if self.finder_mode == FinderMode::Files {
                    "find"
                } else {
                    "search"
                };
                format!(" | {label}: {}", self.finder_query)
            }
            Mode::Command => format!(" | :{}", self.command_input),
            Mode::ConfirmCreate => {
                if let Some(path) = &self.pending_create_path {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "note.md".to_string());
                    format!(" | create {name}? (y/n)")
                } else {
                    " | create note? (y/n)".to_string()
                }
            }
            _ => String::new(),
        };

        if self.quit_confirm_armed {
            let pending = self.pending_write_count();
            suffix.push_str(&format!(" | {pending} pending, press q again to save+quit"));
        }

        let info = Span::styled(
            format!(
                " {file_name}{dirty_marker}  {}:{}{} ",
                self.buffer.cursor.row + 1,
                self.buffer.cursor.col + 1,
                suffix
            ),
            Style::default().fg(Color::Gray).bg(Color::DarkGray),
        );

        let bar = Line::from(vec![mode_span, info]);
        let status = Paragraph::new(bar).style(Style::default().bg(Color::DarkGray));
        frame.render_widget(status, area);
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
            "  Ctrl+N/P: Tabs  Ctrl+E: Sidebar  Ctrl+B: Backlinks  gd: Follow Link  /: Finder  Ctrl+Shift+F: Search  : Command (help)  i: Insert  q: Confirm Quit  Q: Save+Quit ",
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
