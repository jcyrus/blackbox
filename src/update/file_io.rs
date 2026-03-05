use crate::app::{App, same_file_path, spawn_buffer_save};
use crate::model::buffer::Buffer;
use anyhow::Result;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

impl App {
    pub(crate) fn handle_tick(&mut self) -> Result<()> {
        let now = Instant::now();

        if self.quit_confirm_until.is_some_and(|until| now >= until) {
            self.quit_confirm_armed = false;
            self.quit_confirm_until = None;
        }

        if let Some(deadline) = self.buffer.save_debounce
            && now >= deadline
        {
            self.save_buffer()?;
        }

        let due_inactive: Vec<PathBuf> = self
            .inactive_buffers
            .iter()
            .filter_map(|(path, buf)| {
                buf.save_debounce
                    .filter(|deadline| now >= *deadline)
                    .map(|_| path.clone())
            })
            .collect();

        for path in due_inactive {
            self.save_inactive_buffer(&path);
        }

        Ok(())
    }
    pub(crate) fn save_buffer(&mut self) -> Result<()> {
        let Some(path) = self.buffer.path.clone() else {
            return Ok(());
        };

        self.save_active_buffer_at_path(path);

        Ok(())
    }
    pub(crate) fn save_active_buffer_at_path(&mut self, path: PathBuf) {
        self.buffer.save_debounce = None;
        self.buffer.dirty = false;
        self.last_saved_file = Some((path.clone(), Instant::now()));

        let rope = self.buffer.rope.clone();
        spawn_buffer_save(path, rope);
    }
    pub(crate) fn save_inactive_buffer(&mut self, path: &PathBuf) {
        let Some(buffer) = self.inactive_buffers.get_mut(path) else {
            return;
        };

        let Some(path) = buffer.path.clone() else {
            return;
        };

        buffer.save_debounce = None;
        buffer.dirty = false;
        let rope = buffer.rope.clone();
        spawn_buffer_save(path, rope);
    }
    pub(crate) fn save_all_buffers(&mut self) {
        if let Some(path) = self.buffer.path.clone()
            && (self.buffer.dirty || self.buffer.save_debounce.is_some())
        {
            self.save_active_buffer_at_path(path);
        }

        let to_save: Vec<PathBuf> = self
            .inactive_buffers
            .iter()
            .filter_map(|(path, buffer)| {
                if buffer.dirty || buffer.save_debounce.is_some() {
                    Some(path.clone())
                } else {
                    None
                }
            })
            .collect();

        for path in to_save {
            self.save_inactive_buffer(&path);
        }
    }
    pub(crate) fn handle_file_changed(&mut self, path: PathBuf) -> Result<()> {
        self.file_tree.refresh()?;

        if !path.exists() {
            self.open_tabs.retain(|tab| !same_file_path(tab, &path));
        }

        let stale_tabs: Vec<PathBuf> = self
            .inactive_buffers
            .keys()
            .filter(|tab_path| same_file_path(tab_path, &path))
            .cloned()
            .collect();
        for stale in stale_tabs {
            self.inactive_buffers.remove(&stale);
        }

        if !self.should_reload_active(&path) {
            return Ok(());
        }

        let old_cursor = self.buffer.cursor.clone();
        let old_viewport = self.buffer.viewport.clone();

        if let Ok(mut reloaded) = Buffer::from_file(path) {
            reloaded.cursor = old_cursor;
            reloaded.viewport = old_viewport;
            reloaded.viewport.scroll_off = self.config.editor.scroll_off;
            reloaded.clamp_cursor();
            reloaded.scroll_to_cursor();
            self.buffer = reloaded;
            self.mark_render_dirty();
        }

        Ok(())
    }
    pub(crate) fn should_reload_active(&self, path: &PathBuf) -> bool {
        let Some(active) = self.buffer.path.as_ref() else {
            return false;
        };

        if !same_file_path(active, path) {
            return false;
        }

        if self.buffer.dirty {
            return false;
        }

        if let Some((saved_path, saved_at)) = &self.last_saved_file {
            let recently_saved =
                Instant::now().duration_since(*saved_at) <= Duration::from_millis(1200);
            if recently_saved && same_file_path(saved_path, path) {
                return false;
            }
        }

        true
    }
    pub(crate) fn open_file(&mut self, path: PathBuf) -> Result<()> {
        if self
            .buffer
            .path
            .as_ref()
            .is_some_and(|current| same_file_path(current, &path))
        {
            return Ok(());
        }

        if self.buffer.dirty {
            self.save_buffer()?;
        }

        self.activate_tab(path)?;

        if self.backlinks_visible {
            self.refresh_backlinks();
        }

        Ok(())
    }
    pub(crate) fn schedule_auto_save(&mut self) {
        let debounce_ms = self.config.general.auto_save_debounce_ms;
        self.buffer.save_debounce = Some(Instant::now() + Duration::from_millis(debounce_ms));
    }
}
