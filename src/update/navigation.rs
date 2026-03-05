use crate::app::{
    App, BacklinkEntry, WIKILINK_RE, parse_wikilink_target, same_file_path, sanitize_link_name,
};
use crate::model::buffer::Buffer;
use crate::model::mode::Mode;
use anyhow::Result;
use std::path::PathBuf;

impl App {
    pub(crate) fn switch_tab_relative(&mut self, delta: isize) -> Result<()> {
        if self.open_tabs.len() <= 1 {
            return Ok(());
        }

        let Some(current_idx) = self.active_tab_index() else {
            return Ok(());
        };

        let len = self.open_tabs.len() as isize;
        let next = (current_idx as isize + delta).rem_euclid(len) as usize;
        let path = self.open_tabs[next].clone();
        self.activate_tab(path)
    }
    pub(crate) fn active_tab_index(&self) -> Option<usize> {
        let active = self.buffer.path.as_ref()?;
        self.open_tabs
            .iter()
            .position(|p| same_file_path(p, active))
    }
    pub(crate) fn activate_tab(&mut self, path: PathBuf) -> Result<()> {
        if let Some(active_path) = self.buffer.path.clone() {
            let current = std::mem::replace(&mut self.buffer, Buffer::new());
            self.inactive_buffers.insert(active_path, current);
        }

        let mut next = if let Some(buf) = self.inactive_buffers.remove(&path) {
            buf
        } else {
            Buffer::from_file(path.clone())?
        };

        next.viewport.scroll_off = self.config.editor.scroll_off;
        self.buffer = next;

        if !self.open_tabs.iter().any(|p| same_file_path(p, &path)) {
            self.open_tabs.push(path);
        }

        if self.backlinks_visible {
            self.refresh_backlinks();
        }

        self.mark_render_dirty();
        Ok(())
    }
    pub(crate) fn toggle_backlinks_panel(&mut self) -> Result<()> {
        self.backlinks_visible = !self.backlinks_visible;

        if self.backlinks_visible {
            self.refresh_backlinks();
            self.mode = Mode::Backlinks;
        } else {
            self.mode = Mode::Normal;
        }

        self.mark_render_dirty();
        self.file_tree.refresh()?;
        Ok(())
    }
    pub(crate) fn refresh_backlinks(&mut self) {
        self.backlinks.clear();
        self.backlinks_selected = 0;

        let Some(active_path) = self.buffer.path.clone() else {
            return;
        };

        let Some(note_name) = active_path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
        else {
            return;
        };

        let files = self.file_tree.all_file_paths();
        for path in files {
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }
            if same_file_path(&path, &active_path) {
                continue;
            }

            let Ok(contents) = std::fs::read_to_string(&path) else {
                continue;
            };

            for (idx, line) in contents.lines().enumerate() {
                let has_link = WIKILINK_RE.find_iter(line).any(|m| {
                    parse_wikilink_target(&line[m.start()..m.end()])
                        .is_some_and(|target| target.eq_ignore_ascii_case(&note_name))
                });

                if has_link {
                    self.backlinks.push(BacklinkEntry {
                        path: path.clone(),
                        line: idx + 1,
                        preview: line.trim().to_string(),
                    });
                }
            }
        }

        self.backlinks.sort_by(|a, b| {
            a.path
                .cmp(&b.path)
                .then(a.line.cmp(&b.line))
                .then(a.preview.cmp(&b.preview))
        });
    }
    pub(crate) fn follow_wikilink_under_cursor(&mut self) -> Result<()> {
        let Some(link_text) = self.wikilink_under_cursor() else {
            self.notifications
                .push_back("No WikiLink under cursor".to_string());
            return Ok(());
        };

        if let Some(target) = self.resolve_wikilink_target(&link_text) {
            self.open_file(target)?;
            return Ok(());
        }

        let path = self.config.vault_path().join(format!("{link_text}.md"));
        self.pending_create_path = Some(path);
        self.mode = Mode::ConfirmCreate;
        self.mark_render_dirty();
        Ok(())
    }
    pub(crate) fn confirm_create_wikilink(&mut self) -> Result<()> {
        let Some(path) = self.pending_create_path.take() else {
            self.mode = Mode::Normal;
            return Ok(());
        };

        let title = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".to_string());

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if !path.exists() {
            std::fs::write(&path, format!("# {title}\n\n"))?;
        }

        self.mode = Mode::Normal;
        self.file_tree.refresh()?;
        self.open_file(path)?;
        Ok(())
    }
    pub(crate) fn wikilink_under_cursor(&self) -> Option<String> {
        let line = self.buffer.line_text(self.buffer.cursor.row)?;
        let col = self.buffer.cursor.col;

        for m in WIKILINK_RE.find_iter(&line) {
            if col >= m.start() && col < m.end() {
                return parse_wikilink_target(&line[m.start()..m.end()]);
            }
        }

        None
    }
    pub(crate) fn resolve_wikilink_target(&self, link_text: &str) -> Option<PathBuf> {
        let clean = sanitize_link_name(link_text);
        if clean.is_empty() {
            return None;
        }

        let vault = self.config.vault_path();
        let exact = vault.join(format!("{clean}.md"));
        if exact.exists() {
            return Some(exact);
        }

        let expected = format!("{clean}.md").to_lowercase();
        self.file_tree.all_file_paths().into_iter().find(|path| {
            path.extension().and_then(|ext| ext.to_str()) == Some("md")
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().to_lowercase() == expected)
        })
    }
}
