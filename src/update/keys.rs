use crate::app::{App, FinderMode};
use crate::model::mode::Mode;
use crate::msg::{Direction as MoveDir, Msg};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::{Duration, Instant};

impl App {
    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.mode {
            Mode::Normal => self.handle_key_normal(key),
            Mode::Insert => self.handle_key_insert(key),
            Mode::Command => self.handle_key_command(key),
            Mode::Sidebar => self.handle_key_sidebar(key),
            Mode::SidebarCreate => self.handle_key_sidebar_create(key),
            Mode::FinderOpen => self.handle_key_finder(key),
            Mode::ConfirmCreate => self.handle_key_confirm_create(key),
            Mode::Backlinks => self.handle_key_backlinks(key),
            _ => Ok(()),
        }
    }
    pub(crate) fn handle_key_normal(&mut self, key: KeyEvent) -> Result<()> {
        if key.code != KeyCode::Char('q') {
            self.quit_confirm_armed = false;
            self.quit_confirm_until = None;
        }

        if self.pending_key == Some(' ') {
            self.pending_key = None;
            self.pending_key_since = None;
            self.mark_render_dirty();

            match key.code {
                KeyCode::Char('f') => self.open_finder(FinderMode::Files)?,
                KeyCode::Char('g') => self.open_finder(FinderMode::Content)?,
                KeyCode::Char('e') => {
                    self.sidebar_visible = !self.sidebar_visible;
                    if self.sidebar_visible {
                        self.file_tree.refresh()?;
                        self.mode = Mode::Sidebar;
                    }
                }
                KeyCode::Char('b') => self.toggle_backlinks_panel()?,
                KeyCode::Char('n') => {
                    self.file_tree.begin_create();
                    self.sidebar_visible = true;
                    self.mode = Mode::SidebarCreate;
                }
                KeyCode::Char('p') => {
                    self.mode = Mode::Command;
                    self.command_input.clear();
                    self.command_input.push_str("plugins");
                    self.mark_render_dirty();
                }
                KeyCode::Char('h') => {
                    let _ = self.event_tx.send(Msg::PluginCommand("help".to_string()));
                }
                _ => {}
            }
            return Ok(());
        }

        if self.pending_key == Some('g') {
            self.pending_key = None;
            if key.code == KeyCode::Char('d') {
                self.follow_wikilink_under_cursor()?;
                return Ok(());
            } else if key.code == KeyCode::Char('g') {
                self.move_cursor(MoveDir::Top);
                return Ok(());
            } else if key.code == KeyCode::Char('t') {
                self.switch_tab_relative(1)?;
                return Ok(());
            } else if key.code == KeyCode::Char('T') {
                self.switch_tab_relative(-1)?;
                return Ok(());
            }
        }

        if self.pending_key == Some('d') {
            self.pending_key = None;
            if key.code == KeyCode::Char('d') {
                self.buffer.delete_line(self.buffer.cursor.row);
                self.buffer.clamp_cursor();
                self.mark_render_dirty();
                self.schedule_auto_save();
                return Ok(());
            }
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('e') {
            self.sidebar_visible = !self.sidebar_visible;
            if self.sidebar_visible {
                self.file_tree.refresh()?;
                self.mode = Mode::Sidebar;
            }
            return Ok(());
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('b') {
            self.toggle_backlinks_panel()?;
            return Ok(());
        }

        match key.code {
            KeyCode::Char(' ') if key.modifiers.is_empty() => {
                self.pending_key = Some(' ');
                self.pending_key_since = Some(Instant::now());
            }
            KeyCode::Char('g') if key.modifiers.is_empty() => {
                self.pending_key = Some('g');
            }
            KeyCode::Char('d') if key.modifiers.is_empty() => {
                self.pending_key = Some('d');
            }
            KeyCode::Char('q') => {
                let pending = self.pending_write_count();
                if pending == 0 {
                    self.should_quit = true;
                } else if self.quit_confirm_armed {
                    self.save_all_buffers();
                    self.should_quit = true;
                    self.quit_confirm_armed = false;
                    self.quit_confirm_until = None;
                } else {
                    self.quit_confirm_armed = true;
                    self.quit_confirm_until = Some(Instant::now() + Duration::from_secs(2));
                }
            }
            KeyCode::Char('Q') => {
                self.save_all_buffers();
                self.should_quit = true;
            }
            // Basic insert
            KeyCode::Char('i') => self.mode = Mode::Insert,
            // Insert variants
            KeyCode::Char('a') => {
                self.move_cursor(MoveDir::Right);
                self.mode = Mode::Insert;
            }
            KeyCode::Char('A') => {
                self.move_cursor(MoveDir::LineEnd);
                self.mode = Mode::Insert;
            }
            KeyCode::Char('I') => {
                self.move_cursor(MoveDir::FirstNonWhitespace);
                self.mode = Mode::Insert;
            }
            KeyCode::Char('o') => {
                self.move_cursor(MoveDir::LineEnd);
                self.buffer.insert_newline();
                self.mode = Mode::Insert;
                self.mark_render_dirty();
                self.schedule_auto_save();
            }
            KeyCode::Char('O') => {
                self.move_cursor(MoveDir::LineStart);
                self.buffer.insert_newline();
                self.move_cursor(MoveDir::Up);
                self.mode = Mode::Insert;
                self.mark_render_dirty();
                self.schedule_auto_save();
            }
            KeyCode::Char('x') => {
                self.buffer.delete_char_forward();
                self.mark_render_dirty();
                self.schedule_auto_save();
            }
            KeyCode::Char(':') => {
                self.mode = Mode::Command;
                self.command_input.clear();
                self.mark_render_dirty();
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.switch_tab_relative(1)?;
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.switch_tab_relative(-1)?;
            }
            KeyCode::Char('/') => self.open_finder(FinderMode::Files)?,
            KeyCode::Char('F')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && key.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                self.open_finder(FinderMode::Content)?;
            }
            KeyCode::Char('f')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && key.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                self.open_finder(FinderMode::Content)?;
            }
            // Motions
            KeyCode::Char('h') | KeyCode::Left => self.move_cursor(MoveDir::Left),
            KeyCode::Char('j') | KeyCode::Down => self.move_cursor(MoveDir::Down),
            KeyCode::Char('k') | KeyCode::Up => self.move_cursor(MoveDir::Up),
            KeyCode::Char('l') | KeyCode::Right => self.move_cursor(MoveDir::Right),
            KeyCode::Char('w') => self.move_cursor(MoveDir::WordForward),
            KeyCode::Char('b') => self.move_cursor(MoveDir::WordBackward),
            KeyCode::Char('e') => self.move_cursor(MoveDir::WordEnd),
            KeyCode::Char('G') => self.move_cursor(MoveDir::Bottom),
            KeyCode::Char('^') => self.move_cursor(MoveDir::FirstNonWhitespace),
            KeyCode::Char('{') => self.move_cursor(MoveDir::ParagraphUp),
            KeyCode::Char('}') => self.move_cursor(MoveDir::ParagraphDown),
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_cursor(MoveDir::PageUp);
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_cursor(MoveDir::PageDown);
            }
            KeyCode::Char('0') => self.move_cursor(MoveDir::LineStart),
            KeyCode::Char('$') => self.move_cursor(MoveDir::LineEnd),
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.save_buffer()?;
            }
            _ => {}
        }
        Ok(())
    }
    pub(crate) fn handle_key_command(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.command_input.clear();
                self.mark_render_dirty();
            }
            KeyCode::Enter => {
                let command = self.command_input.trim().to_string();
                self.mode = Mode::Normal;
                self.command_input.clear();
                self.mark_render_dirty();

                if !command.is_empty() {
                    let _ = self.event_tx.send(Msg::PluginCommand(command));
                }
            }
            KeyCode::Backspace => {
                self.command_input.pop();
                self.mark_render_dirty();
            }
            KeyCode::Char(ch)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.command_input.push(ch);
                self.mark_render_dirty();
            }
            _ => {}
        }

        Ok(())
    }
    pub(crate) fn handle_key_insert(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('e') {
            self.sidebar_visible = !self.sidebar_visible;
            if self.sidebar_visible {
                self.file_tree.refresh()?;
                self.mode = Mode::Sidebar;
            }
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Enter => {
                self.buffer.insert_newline();
                self.mark_render_dirty();
                self.schedule_auto_save();
            }
            KeyCode::Backspace => {
                self.buffer.delete_char_before();
                self.mark_render_dirty();
                self.schedule_auto_save();
            }
            KeyCode::Char(ch) => {
                self.buffer.insert_char(ch);
                self.mark_render_dirty();
                self.schedule_auto_save();
            }
            KeyCode::Left => self.move_cursor(MoveDir::Left),
            KeyCode::Right => self.move_cursor(MoveDir::Right),
            KeyCode::Up => self.move_cursor(MoveDir::Up),
            KeyCode::Down => self.move_cursor(MoveDir::Down),
            _ => {}
        }
        Ok(())
    }
    pub(crate) fn handle_key_sidebar(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('e') {
            self.sidebar_visible = false;
            self.mode = Mode::Normal;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
            }
            KeyCode::Char('j') | KeyCode::Down => self.file_tree.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.file_tree.move_selection(-1),
            KeyCode::Char('h') | KeyCode::Left => self.file_tree.collapse_selected_or_parent()?,
            KeyCode::Char('l') | KeyCode::Right => {
                if let Some(node) = self.file_tree.selected_node() {
                    if node.is_dir {
                        self.file_tree.toggle_selected_dir()?;
                    } else {
                        self.open_file(node.path.clone())?;
                        self.mode = Mode::Normal;
                    }
                }
            }
            KeyCode::Char('a') => {
                self.file_tree.begin_create();
                self.mode = Mode::SidebarCreate;
            }
            KeyCode::Enter => {
                if let Some(node) = self.file_tree.selected_node() {
                    if node.is_dir {
                        self.file_tree.toggle_selected_dir()?;
                    } else {
                        self.open_file(node.path.clone())?;
                        self.mode = Mode::Normal;
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }
    pub(crate) fn handle_key_confirm_create(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.confirm_create_wikilink()?;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.pending_create_path = None;
                self.mode = Mode::Normal;
            }
            _ => {}
        }
        Ok(())
    }
    pub(crate) fn handle_key_backlinks(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('b') {
            self.toggle_backlinks_panel()?;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.backlinks_visible = false;
                self.mode = Mode::Normal;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.backlinks.is_empty() {
                    self.backlinks_selected =
                        (self.backlinks_selected + 1).min(self.backlinks.len().saturating_sub(1));
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.backlinks_selected = self.backlinks_selected.saturating_sub(1);
            }
            KeyCode::Enter => {
                if let Some(entry) = self.backlinks.get(self.backlinks_selected).cloned() {
                    self.open_file(entry.path)?;
                    let target = entry.line.saturating_sub(1);
                    self.buffer.cursor.row = target.min(self.buffer.line_count().saturating_sub(1));
                    self.buffer.cursor.col = 0;
                    self.buffer.cursor.desired_col = 0;
                    self.buffer.scroll_to_cursor();
                }
            }
            _ => {}
        }

        Ok(())
    }
    pub(crate) fn handle_key_sidebar_create(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.file_tree.create_input.clear();
                self.mode = Mode::Sidebar;
            }
            KeyCode::Backspace => {
                self.file_tree.create_input.pop();
            }
            KeyCode::Enter => {
                if let Some(path) = self.file_tree.commit_create()? {
                    self.open_file(path)?;
                    self.mode = Mode::Normal;
                } else {
                    self.mode = Mode::Sidebar;
                }
            }
            KeyCode::Char(ch) => {
                self.file_tree.create_input.push(ch);
            }
            _ => {}
        }
        Ok(())
    }
    pub(crate) fn handle_key_finder(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.finder_query.clear();
                self.finder_results.clear();
                self.finder_selected = 0;
            }
            KeyCode::Enter => {
                if let Some(result) = self.finder_results.get(self.finder_selected).cloned() {
                    self.open_file(result.path)?;
                    if let Some(line) = result.line {
                        let target = line.saturating_sub(1);
                        self.buffer.cursor.row =
                            target.min(self.buffer.line_count().saturating_sub(1));
                        self.buffer.cursor.col = 0;
                        self.buffer.cursor.desired_col = 0;
                        self.buffer.scroll_to_cursor();
                    }
                }
                self.mode = Mode::Normal;
                self.finder_query.clear();
                self.finder_results.clear();
                self.finder_selected = 0;
            }
            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if !self.finder_results.is_empty() {
                    self.finder_selected =
                        (self.finder_selected + 1).min(self.finder_results.len() - 1);
                }
            }
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if !self.finder_results.is_empty() {
                    self.finder_selected = self.finder_selected.saturating_sub(1);
                }
            }
            KeyCode::Backspace => {
                self.finder_query.pop();
                self.refresh_finder_results()?;
            }
            KeyCode::Char(ch)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.finder_query.push(ch);
                self.refresh_finder_results()?;
            }
            _ => {}
        }

        Ok(())
    }
}
