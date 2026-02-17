use crossterm::event::{KeyEvent, MouseEvent};
use std::path::PathBuf;

/// Direction for cursor movement.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
    LineStart,
    LineEnd,
}

/// All possible messages that drive state transitions.
#[derive(Debug)]
#[allow(dead_code)]
pub enum Msg {
    // -- Input events (raw)
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),

    // -- Buffer operations
    InsertChar(char),
    DeleteChar,
    NewLine,
    MoveCursor(Direction),

    // -- Mode
    SetMode(crate::model::mode::Mode),

    // -- File I/O
    SaveActiveBuffer,
    SaveAllBuffers,
    OpenFile(PathBuf),
    FileChanged(PathBuf),
    ScratchAutoSave,

    // -- System
    Tick,
    Quit,
}
