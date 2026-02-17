use ropey::Rope;
use std::path::PathBuf;
use std::time::Instant;

use super::cursor::CursorState;

/// Viewport state for scroll tracking.
#[derive(Debug, Clone)]
pub struct Viewport {
    pub top_line: usize,
    pub height: u16,
    pub scroll_off: u16,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            top_line: 0,
            height: 24,
            scroll_off: 5,
        }
    }
}

/// A single text buffer backed by a Rope.
pub struct Buffer {
    pub rope: Rope,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub cursor: CursorState,
    pub viewport: Viewport,
    pub save_debounce: Option<Instant>,
}

impl Buffer {
    /// Create a new empty buffer.
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            path: None,
            dirty: false,
            cursor: CursorState::default(),
            viewport: Viewport::default(),
            save_debounce: None,
        }
    }

    /// Create a buffer from file contents.
    pub fn from_file(path: PathBuf) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(&path)?;
        Ok(Self {
            rope: Rope::from_str(&text),
            path: Some(path),
            dirty: false,
            cursor: CursorState::default(),
            viewport: Viewport::default(),
            save_debounce: None,
        })
    }

    /// Total number of lines in the buffer.
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// Get the text of a specific line (without trailing newline).
    pub fn line_text(&self, idx: usize) -> Option<String> {
        if idx >= self.rope.len_lines() {
            return None;
        }
        let line = self.rope.line(idx);
        let mut s: String = line.chunks().collect();
        if s.ends_with('\n') {
            s.pop();
        }
        if s.ends_with('\r') {
            s.pop();
        }
        Some(s)
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, ch: char) {
        let byte_idx = self.cursor_byte_offset();
        self.rope.insert_char(byte_idx, ch);
        self.cursor.col += ch.len_utf8();
        self.dirty = true;
    }

    /// Insert a newline at the cursor position.
    pub fn insert_newline(&mut self) {
        let byte_idx = self.cursor_byte_offset();
        self.rope.insert_char(byte_idx, '\n');
        self.cursor.row += 1;
        self.cursor.col = 0;
        self.cursor.desired_col = 0;
        self.dirty = true;
    }

    /// Delete the character before the cursor (backspace).
    pub fn delete_char_before(&mut self) {
        if self.cursor.col == 0 && self.cursor.row == 0 {
            return;
        }

        if self.cursor.col == 0 {
            // Join with previous line
            let prev_line_len = self
                .line_text(self.cursor.row - 1)
                .map(|l| l.len())
                .unwrap_or(0);
            let byte_idx = self.cursor_byte_offset();
            // delete the newline at end of previous line
            self.rope.remove(byte_idx - 1..byte_idx);
            self.cursor.row -= 1;
            self.cursor.col = prev_line_len;
        } else {
            let byte_idx = self.cursor_byte_offset();
            // Find the previous character boundary
            let prev_char_len = self
                .rope
                .byte_slice(..byte_idx)
                .chunks()
                .last()
                .and_then(|s| s.chars().next_back())
                .map(|c| c.len_utf8())
                .unwrap_or(1);
            self.rope.remove(byte_idx - prev_char_len..byte_idx);
            self.cursor.col -= prev_char_len;
        }

        self.dirty = true;
    }

    /// Compute the byte offset in the rope for the current cursor position.
    fn cursor_byte_offset(&self) -> usize {
        let line_start = self.rope.line_to_byte(self.cursor.row);
        line_start + self.cursor.col
    }

    /// Ensure the cursor stays within valid bounds.
    pub fn clamp_cursor(&mut self) {
        let max_row = self.rope.len_lines().saturating_sub(1);
        self.cursor.row = self.cursor.row.min(max_row);

        let line_len = self
            .line_text(self.cursor.row)
            .map(|l| l.len())
            .unwrap_or(0);
        self.cursor.col = self.cursor.col.min(line_len);
    }

    /// Ensure the viewport keeps the cursor visible.
    pub fn scroll_to_cursor(&mut self) {
        let off = self.viewport.scroll_off as usize;
        let height = self.viewport.height as usize;

        if self.cursor.row < self.viewport.top_line + off {
            self.viewport.top_line = self.cursor.row.saturating_sub(off);
        }
        if self.cursor.row >= self.viewport.top_line + height - off {
            self.viewport.top_line = self.cursor.row + off + 1 - height;
        }
    }
}
