use ropey::Rope;
use std::path::PathBuf;
use std::time::Instant;

use super::cursor::CursorState;

#[derive(Debug, Clone)]
pub struct UndoEntry {
    pub rope: Rope,
    pub cursor: CursorState,
}

#[derive(Debug, Clone)]
pub struct UndoTree {
    pub history: Vec<UndoEntry>,
    pub redo_stack: Vec<UndoEntry>,
    pub last_edit_time: Instant,
}

impl Default for UndoTree {
    fn default() -> Self {
        Self {
            history: Vec::new(),
            redo_stack: Vec::new(),
            last_edit_time: Instant::now(),
        }
    }
}

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
    pub undo_tree: UndoTree,
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
            undo_tree: UndoTree::default(),
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
            undo_tree: UndoTree::default(),
        })
    }

    pub fn push_snapshot(&mut self) {
        let now = Instant::now();
        if now
            .duration_since(self.undo_tree.last_edit_time)
            .as_millis()
            > 500
        {
            self.undo_tree.history.push(UndoEntry {
                rope: self.rope.clone(),
                cursor: self.cursor.clone(),
            });
            self.undo_tree.redo_stack.clear();
        }
        self.undo_tree.last_edit_time = now;
    }

    pub fn undo(&mut self) -> bool {
        if let Some(entry) = self.undo_tree.history.pop() {
            self.undo_tree.redo_stack.push(UndoEntry {
                rope: self.rope.clone(),
                cursor: self.cursor.clone(),
            });
            self.rope = entry.rope;
            self.cursor = entry.cursor;
            self.dirty = true;
            return true;
        }
        false
    }

    pub fn redo(&mut self) -> bool {
        if let Some(entry) = self.undo_tree.redo_stack.pop() {
            self.undo_tree.history.push(UndoEntry {
                rope: self.rope.clone(),
                cursor: self.cursor.clone(),
            });
            self.rope = entry.rope;
            self.cursor = entry.cursor;
            self.dirty = true;
            return true;
        }
        false
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
        self.push_snapshot();
        let byte_idx = self.cursor_byte_offset();
        self.rope.insert_char(byte_idx, ch);
        self.cursor.col += ch.len_utf8();
        self.dirty = true;
    }

    /// Insert a newline at the cursor position.
    pub fn insert_newline(&mut self) {
        self.push_snapshot();
        let byte_idx = self.cursor_byte_offset();
        self.rope.insert_char(byte_idx, '\n');
        self.cursor.row += 1;
        self.cursor.col = 0;
        self.cursor.desired_col = 0;
        self.dirty = true;
    }

    /// Delete the character before the cursor (backspace).
    pub fn delete_char_before(&mut self) {
        self.push_snapshot();
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

    pub fn delete_char_forward(&mut self) {
        self.push_snapshot();
        if self.cursor.row >= self.line_count() {
            return;
        }
        let line_len = self
            .line_text(self.cursor.row)
            .map(|l| l.len())
            .unwrap_or(0);

        if self.cursor.col >= line_len {
            if self.cursor.row < self.line_count().saturating_sub(1) {
                // delete the newline at end of current line
                let byte_idx = self.cursor_byte_offset();
                self.rope.remove(byte_idx..byte_idx + 1);
            }
        } else {
            let byte_idx = self.cursor_byte_offset();
            // Find the next character boundary
            let next_char_len = self
                .rope
                .byte_slice(byte_idx..)
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(1);
            self.rope.remove(byte_idx..byte_idx + next_char_len);
        }

        self.dirty = true;
    }

    pub fn delete_line(&mut self, row: usize) {
        self.push_snapshot();
        if row >= self.line_count() {
            return;
        }
        let start_idx = self.rope.line_to_byte(row);
        let end_idx = if row + 1 < self.line_count() {
            self.rope.line_to_byte(row + 1)
        } else {
            self.rope.len_bytes()
        };

        if start_idx < self.rope.len_bytes() {
            self.rope.remove(start_idx..end_idx);
        }

        // Let cursor be clamped automatically or explicitly later
        if self.cursor.row > 0 && self.cursor.row >= self.line_count() {
            self.cursor.row -= 1;
        }
        self.cursor.col = 0;
        self.cursor.desired_col = 0;
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

    /// Count the total number of words in the buffer.
    pub fn word_count(&self) -> usize {
        let mut count = 0;
        for i in 0..self.line_count() {
            if let Some(text) = self.line_text(i) {
                count += text.split_whitespace().count();
            }
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_new_buffer_is_empty() {
        let buf = Buffer::new();
        assert_eq!(buf.line_count(), 1); // ropey always has at least 1 line
        assert_eq!(buf.line_text(0), Some(String::new()));
        assert!(!buf.dirty);
        assert_eq!(buf.cursor.row, 0);
        assert_eq!(buf.cursor.col, 0);
    }

    #[test]
    fn test_insert_char_advances_col() {
        let mut buf = Buffer::new();
        buf.insert_char('h');
        buf.insert_char('i');
        assert_eq!(buf.cursor.col, 2);
        assert!(buf.dirty);
        assert_eq!(buf.line_text(0), Some("hi".to_string()));
    }

    #[test]
    fn test_insert_newline_advances_row() {
        let mut buf = Buffer::new();
        buf.insert_char('a');
        buf.insert_newline();
        assert_eq!(buf.cursor.row, 1);
        assert_eq!(buf.cursor.col, 0);
        assert_eq!(buf.line_text(0), Some("a".to_string()));
        assert_eq!(buf.line_text(1), Some(String::new()));
    }

    #[test]
    fn test_delete_char_middle_of_line() {
        let mut buf = Buffer::new();
        buf.insert_char('a');
        buf.insert_char('b');
        buf.insert_char('c');
        // cursor is now at col 3, delete 'c'
        buf.delete_char_before();
        assert_eq!(buf.cursor.col, 2);
        assert_eq!(buf.line_text(0), Some("ab".to_string()));
    }

    #[test]
    fn test_delete_at_start_of_buffer_is_noop() {
        let mut buf = Buffer::new();
        buf.delete_char_before(); // should do nothing
        assert_eq!(buf.cursor.row, 0);
        assert_eq!(buf.cursor.col, 0);
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn test_delete_at_line_start_joins_lines() {
        let mut buf = Buffer::new();
        buf.insert_char('a');
        buf.insert_newline();
        // cursor is at row=1, col=0 — backspace should join lines
        buf.delete_char_before();
        assert_eq!(buf.cursor.row, 0);
        assert_eq!(buf.cursor.col, 1); // end of "a"
        assert_eq!(buf.line_text(0), Some("a".to_string()));
    }

    #[test]
    fn test_clamp_cursor_out_of_bounds() {
        let mut buf = Buffer::new();
        buf.insert_char('x');
        // Manually put cursor way out of bounds
        buf.cursor.row = 999;
        buf.cursor.col = 999;
        buf.clamp_cursor();
        assert_eq!(buf.cursor.row, 0);
        assert_eq!(buf.cursor.col, 1); // length of "x"
    }

    #[test]
    fn test_scroll_to_cursor_scrolls_down() {
        let mut buf = Buffer::new();
        // Create 30 lines
        for i in 0..29 {
            buf.insert_char('a');
            if i < 28 {
                buf.insert_newline();
            }
        }
        buf.viewport.height = 10;
        buf.viewport.scroll_off = 2;
        buf.viewport.top_line = 0;
        buf.cursor.row = 28; // past the visible area
        buf.scroll_to_cursor();
        // top_line should have advanced to keep cursor visible
        assert!(
            buf.viewport.top_line > 0,
            "viewport should have scrolled, top_line={}",
            buf.viewport.top_line
        );
    }

    #[test]
    fn test_from_file_roundtrip() {
        let content = "# Hello\n\nThis is a test note.\n";
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(content.as_bytes()).unwrap();
        let path = tmp.path().to_path_buf();

        let buf = Buffer::from_file(path).unwrap();
        assert_eq!(buf.line_text(0), Some("# Hello".to_string()));
        assert_eq!(buf.line_text(1), Some(String::new()));
        assert_eq!(buf.line_text(2), Some("This is a test note.".to_string()));
        assert!(!buf.dirty);
    }
}
