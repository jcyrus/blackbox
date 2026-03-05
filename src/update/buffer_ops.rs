use crate::app::App;
use crate::msg::Direction as MoveDir;

fn char_class(c: char) -> u8 {
    if c.is_whitespace() {
        0
    } else if c.is_alphanumeric() || c == '_' {
        1
    } else {
        2
    }
}

impl App {
    pub(crate) fn move_cursor(&mut self, dir: MoveDir) {
        let prev_top = self.buffer.viewport.top_line;
        match dir {
            MoveDir::Up => {
                if self.buffer.cursor.row > 0 {
                    self.buffer.cursor.row -= 1;
                    self.buffer.cursor.col = self.buffer.cursor.desired_col;
                }
            }
            MoveDir::Down => {
                if self.buffer.cursor.row < self.buffer.line_count().saturating_sub(1) {
                    self.buffer.cursor.row += 1;
                    self.buffer.cursor.col = self.buffer.cursor.desired_col;
                }
            }
            MoveDir::Left => {
                if self.buffer.cursor.col > 0 {
                    self.buffer.cursor.col -= 1;
                    self.buffer.cursor.desired_col = self.buffer.cursor.col;
                }
            }
            MoveDir::Right => {
                let line_len = self
                    .buffer
                    .line_text(self.buffer.cursor.row)
                    .map(|l| l.len())
                    .unwrap_or(0);
                if self.buffer.cursor.col < line_len {
                    self.buffer.cursor.col += 1;
                    self.buffer.cursor.desired_col = self.buffer.cursor.col;
                }
            }
            MoveDir::LineStart => {
                self.buffer.cursor.col = 0;
                self.buffer.cursor.desired_col = 0;
            }
            MoveDir::LineEnd => {
                let line_len = self
                    .buffer
                    .line_text(self.buffer.cursor.row)
                    .map(|l| l.len())
                    .unwrap_or(0);
                self.buffer.cursor.col = line_len;
                self.buffer.cursor.desired_col = line_len;
            }
            MoveDir::FirstNonWhitespace => {
                let text = self
                    .buffer
                    .line_text(self.buffer.cursor.row)
                    .unwrap_or_default();
                let first_non_ws = text.chars().position(|c| !c.is_whitespace()).unwrap_or(0);
                self.buffer.cursor.col = first_non_ws;
                self.buffer.cursor.desired_col = first_non_ws;
            }
            MoveDir::Top => {
                self.buffer.cursor.row = 0;
                self.buffer.cursor.col = 0;
                self.buffer.cursor.desired_col = 0;
            }
            MoveDir::Bottom => {
                self.buffer.cursor.row = self.buffer.line_count().saturating_sub(1);
                let text = self
                    .buffer
                    .line_text(self.buffer.cursor.row)
                    .unwrap_or_default();
                let first_non_ws = text.chars().position(|c| !c.is_whitespace()).unwrap_or(0);
                self.buffer.cursor.col = first_non_ws;
                self.buffer.cursor.desired_col = first_non_ws;
            }
            MoveDir::PageUp => {
                let visible_height = self
                    .render_cache
                    .bottom
                    .saturating_sub(self.render_cache.top);
                let jump = (visible_height / 2).max(1);
                self.buffer.cursor.row = self.buffer.cursor.row.saturating_sub(jump);
                self.buffer.cursor.col = self.buffer.cursor.desired_col;
            }
            MoveDir::PageDown => {
                let visible_height = self
                    .render_cache
                    .bottom
                    .saturating_sub(self.render_cache.top);
                let jump = (visible_height / 2).max(1);
                self.buffer.cursor.row =
                    (self.buffer.cursor.row + jump).min(self.buffer.line_count().saturating_sub(1));
                self.buffer.cursor.col = self.buffer.cursor.desired_col;
            }
            MoveDir::ParagraphUp => {
                let mut r = self.buffer.cursor.row;
                // Skiping initial empty lines
                while r > 0
                    && self
                        .buffer
                        .line_text(r - 1)
                        .unwrap_or_default()
                        .trim()
                        .is_empty()
                {
                    r -= 1;
                }
                while r > 0
                    && !self
                        .buffer
                        .line_text(r - 1)
                        .unwrap_or_default()
                        .trim()
                        .is_empty()
                {
                    r -= 1;
                }
                self.buffer.cursor.row = r;
                self.buffer.cursor.col = 0;
                self.buffer.cursor.desired_col = 0;
            }
            MoveDir::ParagraphDown => {
                let mut r = self.buffer.cursor.row;
                let max_r = self.buffer.line_count().saturating_sub(1);
                // Skip initial empty lines
                while r < max_r
                    && self
                        .buffer
                        .line_text(r + 1)
                        .unwrap_or_default()
                        .trim()
                        .is_empty()
                {
                    r += 1;
                }
                while r < max_r
                    && !self
                        .buffer
                        .line_text(r + 1)
                        .unwrap_or_default()
                        .trim()
                        .is_empty()
                {
                    r += 1;
                }
                self.buffer.cursor.row = r;
                self.buffer.cursor.col = 0;
                self.buffer.cursor.desired_col = 0;
            }
            MoveDir::WordForward => self.word_forward(),
            MoveDir::WordBackward => self.word_backward(),
            MoveDir::WordEnd => self.word_end(),
        }
        self.buffer.clamp_cursor();
        self.buffer.scroll_to_cursor();
        if self.buffer.viewport.top_line != prev_top {
            self.mark_render_dirty();
        }
    }

    fn word_forward(&mut self) {
        let max_r = self.buffer.line_count().saturating_sub(1);
        let mut row = self.buffer.cursor.row;
        let mut col = self.buffer.cursor.col;

        // Ensure we don't start out of bounds
        let mut text = self.buffer.line_text(row).unwrap_or_default();
        if col >= text.len() && row < max_r {
            row += 1;
            col = 0;
            text = self.buffer.line_text(row).unwrap_or_default();
        }

        if text.is_empty() {
            if row < max_r {
                self.buffer.cursor.row = row + 1;
                self.buffer.cursor.col = 0;
                self.buffer.cursor.desired_col = 0;
            }
            return;
        }

        let chars: Vec<char> = text.chars().collect();
        if col >= chars.len() {
            // End of line, jump to next line
            if row < max_r {
                self.buffer.cursor.row = row + 1;
                self.buffer.cursor.col = 0;
                self.buffer.cursor.desired_col = 0;
            }
            return;
        }

        let start_class = char_class(chars[col]);

        // 1. skip current class chars
        while col < chars.len() && char_class(chars[col]) == start_class && start_class != 0 {
            col += 1;
        }

        // 2. skip whitespace
        while col < chars.len() && char_class(chars[col]) == 0 {
            col += 1;
        }

        if col >= chars.len() {
            // We ran out of line, jump to next line start
            if row < max_r {
                self.buffer.cursor.row = row + 1;
                self.buffer.cursor.col = 0;
            } else {
                self.buffer.cursor.col = chars.len();
            }
        } else {
            self.buffer.cursor.col = col;
        }
        self.buffer.cursor.desired_col = self.buffer.cursor.col;
    }

    fn word_backward(&mut self) {
        let mut row = self.buffer.cursor.row;
        let mut col = self.buffer.cursor.col;

        let mut text = self.buffer.line_text(row).unwrap_or_default();
        let mut chars: Vec<char> = text.chars().collect();

        if col == 0 {
            if row > 0 {
                row -= 1;
                text = self.buffer.line_text(row).unwrap_or_default();
                chars = text.chars().collect();
                col = chars.len();
            } else {
                return;
            }
        } else if col > chars.len() {
            col = chars.len();
        }

        // 1. Step left once
        col = col.saturating_sub(1);

        // 2. skip whitespace backward
        while col > 0 && char_class(chars[col]) == 0 {
            col -= 1;
        }

        let target_class = char_class(chars[col]);
        // 3. skip current class backward
        while col > 0 && char_class(chars[col - 1]) == target_class {
            col -= 1;
        }

        self.buffer.cursor.row = row;
        self.buffer.cursor.col = col;
        self.buffer.cursor.desired_col = col;
    }

    fn word_end(&mut self) {
        let max_r = self.buffer.line_count().saturating_sub(1);
        let mut row = self.buffer.cursor.row;
        let mut col = self.buffer.cursor.col;

        let mut text = self.buffer.line_text(row).unwrap_or_default();
        let mut chars: Vec<char> = text.chars().collect();

        if col >= chars.len() {
            if row < max_r {
                row += 1;
                col = 0;
                text = self.buffer.line_text(row).unwrap_or_default();
                chars = text.chars().collect();
            } else {
                return;
            }
        }

        // 1. Step right once
        col += 1;

        // 2. skip whitespace forward
        while col < chars.len() && char_class(chars[col]) == 0 {
            col += 1;
        }

        if col >= chars.len() {
            // End of line, jump to next line start
            if row < max_r {
                row += 1;
                col = 0;
                text = self.buffer.line_text(row).unwrap_or_default();
                chars = text.chars().collect();
                while col < chars.len() && char_class(chars[col]) == 0 {
                    col += 1;
                }
            }
            if col >= chars.len() {
                self.buffer.cursor.row = row;
                self.buffer.cursor.col = chars.len().saturating_sub(1);
                self.buffer.cursor.desired_col = self.buffer.cursor.col;
                return;
            }
        }

        let target_class = char_class(chars[col]);
        // 3. skip current class forward to its end
        while col + 1 < chars.len() && char_class(chars[col + 1]) == target_class {
            col += 1;
        }

        self.buffer.cursor.row = row;
        self.buffer.cursor.col = col;
        self.buffer.cursor.desired_col = col;
    }
}
