use crossterm::event::{KeyEvent, MouseEvent};
use std::path::PathBuf;

use crate::plugin::manifest::PluginId;

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

    // -- Plugins
    PluginCommand(String),
    PluginEvent(PluginId, PluginAction),

    // -- System
    Tick,
    Quit,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // Phase 3 scaffolding: plugin actions are emitted once runtime callbacks are wired.
pub enum PluginAction {
    Notify(String),
    RequestRedraw,
}
