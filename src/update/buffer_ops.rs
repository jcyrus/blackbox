use crate::app::App;
use crate::msg::Direction as MoveDir;

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
        }
        self.buffer.clamp_cursor();
        self.buffer.scroll_to_cursor();
        if self.buffer.viewport.top_line != prev_top {
            self.mark_render_dirty();
        }
    }
}
