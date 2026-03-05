pub mod buffer_ops;
pub mod file_io;
pub mod keys;
pub mod navigation;
pub mod search;

use crate::app::{App, parse_plugin_command_input};
use crate::msg::{Msg, PluginAction};
use crate::plugin::PluginManager;
use anyhow::Result;

impl App {
    pub fn update(&mut self, msg: Msg) -> Result<()> {
        match msg {
            Msg::Key(key) => self.handle_key(key)?,
            Msg::InsertChar(ch) => {
                self.buffer.insert_char(ch);
                self.mark_render_dirty();
                self.schedule_auto_save();
            }
            Msg::DeleteChar => {
                self.buffer.delete_char_before();
                self.mark_render_dirty();
                self.schedule_auto_save();
            }
            Msg::NewLine => {
                self.buffer.insert_newline();
                self.mark_render_dirty();
                self.schedule_auto_save();
            }
            Msg::MoveCursor(dir) => self.move_cursor(dir),
            Msg::SetMode(mode) => self.mode = mode,
            Msg::SaveActiveBuffer => self.save_buffer()?,
            Msg::SaveAllBuffers => self.save_all_buffers(),
            Msg::OpenFile(path) => self.open_file(path)?,
            Msg::FileChanged(path) => self.handle_file_changed(path)?,
            Msg::PluginCommand(command) => self.handle_plugin_command(command),
            Msg::PluginEvent(_plugin_id, action) => self.handle_plugin_event(action),
            Msg::Tick => self.handle_tick()?,
            Msg::Quit => self.should_quit = true,
            Msg::Resize(_w, h) => {
                self.buffer.viewport.height = h.saturating_sub(3); // tab + status bar
                self.mark_render_dirty();
            }
            _ => {}
        }
        Ok(())
    }
    pub(crate) fn handle_plugin_command(&mut self, command: String) {
        let command = command.trim();
        if command.is_empty() {
            return;
        }

        let notifications = if let Some(raw_plugin_command) = command
            .strip_prefix("plugin ")
            .or_else(|| command.strip_prefix("p "))
        {
            let plugin_command = parse_plugin_command_input(raw_plugin_command);
            if plugin_command.is_empty() {
                vec!["usage: plugin <command> (alias: p <command>)".to_string()]
            } else {
                self.plugin_manager.execute_command(&plugin_command)
            }
        } else {
            match command {
                "help" => {
                    let mut notes = vec!["built-ins:".to_string()];
                    notes.push("  help".to_string());
                    notes.push("  plugin <command> (alias: p <command>)".to_string());
                    notes.push(
                        "    examples: plugin word_count | plugin \"word count\"".to_string(),
                    );
                    notes.push("  plugins (alias: pl)".to_string());
                    notes.push("  plugins.list (alias: pl.list)".to_string());
                    notes.push("  plugins.errors (alias: pl.errors)".to_string());
                    notes.push("  plugins.reload (alias: pl.reload)".to_string());
                    notes.extend(self.plugin_manager.command_notifications());
                    notes
                }
                "plugins" | "pl" => vec![self.plugin_manager.summary_notification()],
                "plugins.list" | "pl.list" => self.plugin_manager.list_notifications(),
                "plugins.errors" | "pl.errors" => {
                    let errors = self.plugin_manager.error_notifications();
                    if errors.is_empty() {
                        vec!["plugins: no errors".to_string()]
                    } else {
                        errors
                    }
                }
                "plugins.reload" | "pl.reload" => {
                    self.plugin_manager = PluginManager::new(&self.config);
                    let mut notes = vec!["plugins: reloaded".to_string()];
                    notes.push(self.plugin_manager.summary_notification());
                    notes.extend(self.plugin_manager.error_notifications());
                    notes
                }
                _ => self.plugin_manager.execute_command(command),
            }
        };

        for notification in notifications {
            self.push_notification(notification);
        }
    }
    pub(crate) fn handle_plugin_event(&mut self, action: PluginAction) {
        match action {
            PluginAction::Notify(message) => self.push_notification(message),
            PluginAction::RequestRedraw => self.mark_render_dirty(),
        }
    }
    pub(crate) fn push_notification(&mut self, message: String) {
        self.notifications.push_back(message);
        while self.notifications.len() > 8 {
            self.notifications.pop_front();
        }
    }
}
