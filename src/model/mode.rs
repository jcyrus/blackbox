/// Application interaction modes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum Mode {
    /// Normal mode — navigation and commands.
    #[default]
    Normal,
    /// Insert mode — text editing.
    Insert,
    /// Sidebar mode — file tree navigation.
    Sidebar,
    /// Sidebar create mode — inline file/folder creation.
    SidebarCreate,
    /// Command palette (`:` prefix).
    Command,
    /// Fuzzy file finder overlay.
    FinderOpen,
    /// WikiLink autocomplete picker.
    LinkPicker,
}

impl Mode {
    pub fn label(&self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Sidebar => "SIDEBAR",
            Mode::SidebarCreate => "CREATE",
            Mode::Command => "COMMAND",
            Mode::FinderOpen => "FINDER",
            Mode::LinkPicker => "LINK",
        }
    }
}
