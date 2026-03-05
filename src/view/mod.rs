pub mod backlinks;
pub mod command;
pub mod editor;
pub mod finder;
pub mod sidebar;
pub mod statusbar;

use crate::app::App;
use crate::model::mode::Mode;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

impl App {
    pub fn view(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // tab bar
                Constraint::Min(1),    // body
                Constraint::Length(1), // status bar
            ])
            .split(frame.area());

        self.render_tab_bar(frame, chunks[0]);

        let editor_area = if self.sidebar_visible && self.backlinks_visible {
            let body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(22),
                    Constraint::Min(1),
                    Constraint::Percentage(25),
                ])
                .split(chunks[1]);

            self.render_sidebar(frame, body[0]);
            self.render_editor(frame, body[1]);
            self.render_backlinks_panel(frame, body[2]);
            body[1]
        } else if self.sidebar_visible {
            let body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(22), Constraint::Min(1)])
                .split(chunks[1]);

            self.render_sidebar(frame, body[0]);
            self.render_editor(frame, body[1]);
            body[1]
        } else if self.backlinks_visible {
            let body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(1), Constraint::Percentage(25)])
                .split(chunks[1]);

            self.render_editor(frame, body[0]);
            self.render_backlinks_panel(frame, body[1]);
            body[0]
        } else {
            self.render_editor(frame, chunks[1]);
            chunks[1]
        };

        let cursor_x = self.buffer.cursor.col as u16 + editor_area.x;
        let cursor_y =
            (self.buffer.cursor.row - self.buffer.viewport.top_line) as u16 + editor_area.y;
        if cursor_y < editor_area.y + editor_area.height {
            frame.set_cursor_position((cursor_x, cursor_y));
        }

        self.render_status_bar(frame, chunks[2]);

        if self.mode == Mode::FinderOpen {
            self.render_finder_overlay(frame);
        } else if self.mode == Mode::Command {
            self.render_command_overlay(frame);
        }

        if let Some(ch) = self.pending_key {
            if ch == ' ' {
                if let Some(since) = self.pending_key_since {
                    if since.elapsed() > std::time::Duration::from_millis(300) {
                        self.render_which_key(frame);
                    }
                }
            }
        }
    }
}
