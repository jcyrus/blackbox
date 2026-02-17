/// Cursor position and selection state within a buffer.
#[derive(Debug, Clone, Default)]
pub struct CursorState {
    /// Current line (0-indexed).
    pub row: usize,
    /// Current column (0-indexed, byte offset within line).
    pub col: usize,
    /// Desired column for vertical movement ("sticky" column).
    pub desired_col: usize,
    /// Active selection range, if any.
    #[allow(dead_code)] // Phase 2: visual selection mode
    pub selection: Option<(Position, Position)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub struct Position {
    pub row: usize,
    pub col: usize,
}

impl CursorState {
    #[allow(dead_code)] // Phase 2: used for selections
    pub fn position(&self) -> Position {
        Position {
            row: self.row,
            col: self.col,
        }
    }

    #[allow(dead_code)] // Phase 2: used for goto/jump commands
    pub fn move_to(&mut self, row: usize, col: usize) {
        self.row = row;
        self.col = col;
        self.desired_col = col;
    }

    #[allow(dead_code)] // Phase 2: visual selection mode
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }
}
