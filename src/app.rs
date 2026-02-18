use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use regex::Regex;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Theme as SyntectTheme, ThemeSet};
use syntect::parsing::SyntaxSet;

use crate::model::buffer::Buffer;
use crate::model::config::AppConfig;
use crate::model::file_tree::FileTree;
use crate::model::mode::Mode;
use crate::msg::{Direction as MoveDir, Msg, PluginAction};
use crate::plugin::PluginManager;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FinderMode {
    Files,
    Content,
}

#[derive(Debug, Clone)]
struct FinderResult {
    path: PathBuf,
    line: Option<usize>,
    preview: String,
}

#[derive(Debug, Clone)]
struct BacklinkEntry {
    path: PathBuf,
    line: usize,
    preview: String,
}

#[derive(Default)]
struct RenderCache {
    top: usize,
    bottom: usize,
    lines: Vec<Line<'static>>,
    dirty: bool,
}

static WIKILINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[[^\]]+\]\]").expect("valid wikilink regex"));
static LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[[^\]]+\]\([^\)]+\)").expect("valid markdown link regex"));
static INLINE_CODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"`[^`]+`").expect("valid inline code regex"));
static BOLD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\*\*[^*]+\*\*").expect("valid bold regex"));
static ITALIC_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\*[^*\s][^*]*\*").expect("valid italic regex"));
static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static SYNTECT_THEME: LazyLock<SyntectTheme> = LazyLock::new(|| {
    let themes = ThemeSet::load_defaults();
    themes
        .themes
        .get("base16-ocean.dark")
        .cloned()
        .or_else(|| themes.themes.values().next().cloned())
        .expect("at least one syntect theme")
});

pub struct App {
    pub mode: Mode,
    pub buffer: Buffer, // Phase 1: single buffer. Phase 2: BufferManager with SlotMap.
    inactive_buffers: HashMap<PathBuf, Buffer>,
    open_tabs: Vec<PathBuf>,
    pub file_tree: FileTree,
    pub sidebar_visible: bool,
    finder_mode: FinderMode,
    finder_query: String,
    finder_results: Vec<FinderResult>,
    finder_selected: usize,
    command_input: String,
    pub config: AppConfig,
    #[allow(dead_code)]
    // Phase 3 scaffolding: runtime command/event dispatch will read this manager.
    pub plugin_manager: PluginManager,
    pub should_quit: bool,
    #[allow(dead_code)] // Phase 2: plugin system event bus
    pub event_tx: mpsc::Sender<Msg>,
    #[allow(dead_code)] // Phase 2: status bar notifications
    pub notifications: VecDeque<String>,
    render_cache: RenderCache,
    last_saved_file: Option<(PathBuf, Instant)>,
    quit_confirm_armed: bool,
    quit_confirm_until: Option<Instant>,
    pending_key: Option<char>,
    pending_create_path: Option<PathBuf>,
    backlinks_visible: bool,
    backlinks: Vec<BacklinkEntry>,
    backlinks_selected: usize,
    #[allow(dead_code)] // Phase 2: animation tick tracking
    last_tick: Instant,
}

impl App {
    pub fn new(config: AppConfig, event_tx: mpsc::Sender<Msg>) -> Result<Self> {
        std::fs::create_dir_all(config.vault_path())?;

        let scratch_path = config.scratch_path();

        let buffer = if scratch_path.exists() {
            Buffer::from_file(scratch_path)?
        } else {
            // Ensure vault directory exists
            if let Some(parent) = scratch_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut buf = Buffer::new();
            buf.path = Some(scratch_path);
            buf
        };

        let file_tree = FileTree::new(config.vault_path())?;
        let plugin_manager = PluginManager::new(&config);
        let notifications = VecDeque::from(plugin_manager.startup_notifications());

        Ok(Self {
            mode: Mode::Normal,
            buffer,
            inactive_buffers: HashMap::new(),
            open_tabs: Vec::new(),
            file_tree,
            sidebar_visible: false,
            finder_mode: FinderMode::Files,
            finder_query: String::new(),
            finder_results: Vec::new(),
            finder_selected: 0,
            command_input: String::new(),
            plugin_manager,
            config,
            should_quit: false,
            event_tx,
            notifications,
            render_cache: RenderCache {
                dirty: true,
                ..Default::default()
            },
            last_saved_file: None,
            quit_confirm_armed: false,
            quit_confirm_until: None,
            pending_key: None,
            pending_create_path: None,
            backlinks_visible: false,
            backlinks: Vec::new(),
            backlinks_selected: 0,
            last_tick: Instant::now(),
        }
        .with_initial_tab())
    }

    fn with_initial_tab(mut self) -> Self {
        if let Some(path) = self.buffer.path.clone() {
            self.open_tabs.push(path);
        }
        self
    }

    fn pending_write_count(&self) -> usize {
        let mut count = 0;

        if self.buffer.dirty || self.buffer.save_debounce.is_some() {
            count += 1;
        }

        count
            + self
                .inactive_buffers
                .values()
                .filter(|buffer| buffer.dirty || buffer.save_debounce.is_some())
                .count()
    }

    // ── MVU: Update ──────────────────────────────────────────────

    pub fn update(&mut self, msg: Msg) -> Result<()> {
        match msg {
            Msg::Key(key) => self.handle_key(key)?,
            Msg::InsertChar(ch) => {
                self.buffer.insert_char(ch);
                self.mark_render_dirty();
                self.schedule_auto_save();
            }
            Msg::DeleteChar => {
                self.buffer.delete_char_before();
                self.mark_render_dirty();
                self.schedule_auto_save();
            }
            Msg::NewLine => {
                self.buffer.insert_newline();
                self.mark_render_dirty();
                self.schedule_auto_save();
            }
            Msg::MoveCursor(dir) => self.move_cursor(dir),
            Msg::SetMode(mode) => self.mode = mode,
            Msg::SaveActiveBuffer => self.save_buffer()?,
            Msg::SaveAllBuffers => self.save_all_buffers(),
            Msg::OpenFile(path) => self.open_file(path)?,
            Msg::FileChanged(path) => self.handle_file_changed(path)?,
            Msg::PluginCommand(command) => self.handle_plugin_command(command),
            Msg::PluginEvent(_plugin_id, action) => self.handle_plugin_event(action),
            Msg::Tick => self.handle_tick()?,
            Msg::Quit => self.should_quit = true,
            Msg::Resize(_w, h) => {
                self.buffer.viewport.height = h.saturating_sub(3); // tab + status bar
                self.mark_render_dirty();
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_plugin_command(&mut self, command: String) {
        let command = command.trim();
        if command.is_empty() {
            return;
        }

        let notifications = if let Some(raw_plugin_command) = command
            .strip_prefix("plugin ")
            .or_else(|| command.strip_prefix("p "))
        {
            let plugin_command = parse_plugin_command_input(raw_plugin_command);
            if plugin_command.is_empty() {
                vec!["usage: plugin <command> (alias: p <command>)".to_string()]
            } else {
                self.plugin_manager.execute_command(&plugin_command)
            }
        } else {
            match command {
                "help" => {
                    let mut notes = vec!["built-ins:".to_string()];
                    notes.push("  help".to_string());
                    notes.push("  plugin <command> (alias: p <command>)".to_string());
                    notes.push(
                        "    examples: plugin word_count | plugin \"word count\"".to_string(),
                    );
                    notes.push("  plugins (alias: pl)".to_string());
                    notes.push("  plugins.list (alias: pl.list)".to_string());
                    notes.push("  plugins.errors (alias: pl.errors)".to_string());
                    notes.push("  plugins.reload (alias: pl.reload)".to_string());
                    notes.extend(self.plugin_manager.command_notifications());
                    notes
                }
                "plugins" | "pl" => vec![self.plugin_manager.summary_notification()],
                "plugins.list" | "pl.list" => self.plugin_manager.list_notifications(),
                "plugins.errors" | "pl.errors" => {
                    let errors = self.plugin_manager.error_notifications();
                    if errors.is_empty() {
                        vec!["plugins: no errors".to_string()]
                    } else {
                        errors
                    }
                }
                "plugins.reload" | "pl.reload" => {
                    self.plugin_manager = PluginManager::new(&self.config);
                    let mut notes = vec!["plugins: reloaded".to_string()];
                    notes.push(self.plugin_manager.summary_notification());
                    notes.extend(self.plugin_manager.error_notifications());
                    notes
                }
                _ => self.plugin_manager.execute_command(command),
            }
        };

        for notification in notifications {
            self.push_notification(notification);
        }
    }

    fn handle_plugin_event(&mut self, action: PluginAction) {
        match action {
            PluginAction::Notify(message) => self.push_notification(message),
            PluginAction::RequestRedraw => self.mark_render_dirty(),
        }
    }

    fn push_notification(&mut self, message: String) {
        self.notifications.push_back(message);
        while self.notifications.len() > 8 {
            self.notifications.pop_front();
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.mode {
            Mode::Normal => self.handle_key_normal(key),
            Mode::Insert => self.handle_key_insert(key),
            Mode::Command => self.handle_key_command(key),
            Mode::Sidebar => self.handle_key_sidebar(key),
            Mode::SidebarCreate => self.handle_key_sidebar_create(key),
            Mode::FinderOpen => self.handle_key_finder(key),
            Mode::ConfirmCreate => self.handle_key_confirm_create(key),
            Mode::Backlinks => self.handle_key_backlinks(key),
            _ => Ok(()),
        }
    }

    fn handle_key_normal(&mut self, key: KeyEvent) -> Result<()> {
        if key.code != KeyCode::Char('q') {
            self.quit_confirm_armed = false;
            self.quit_confirm_until = None;
        }

        if self.pending_key == Some('g') {
            self.pending_key = None;
            if key.code == KeyCode::Char('d') {
                self.follow_wikilink_under_cursor()?;
                return Ok(());
            }
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('e') {
            self.sidebar_visible = !self.sidebar_visible;
            if self.sidebar_visible {
                self.file_tree.refresh()?;
                self.mode = Mode::Sidebar;
            }
            return Ok(());
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('b') {
            self.toggle_backlinks_panel()?;
            return Ok(());
        }

        match key.code {
            KeyCode::Char('g') if key.modifiers.is_empty() => {
                self.pending_key = Some('g');
            }
            KeyCode::Char('q') => {
                let pending = self.pending_write_count();
                if pending == 0 {
                    self.should_quit = true;
                } else if self.quit_confirm_armed {
                    self.save_all_buffers();
                    self.should_quit = true;
                    self.quit_confirm_armed = false;
                    self.quit_confirm_until = None;
                } else {
                    self.quit_confirm_armed = true;
                    self.quit_confirm_until = Some(Instant::now() + Duration::from_secs(2));
                }
            }
            KeyCode::Char('Q') => {
                self.save_all_buffers();
                self.should_quit = true;
            }
            KeyCode::Char('i') => self.mode = Mode::Insert,
            KeyCode::Char(':') => {
                self.mode = Mode::Command;
                self.command_input.clear();
                self.mark_render_dirty();
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.switch_tab_relative(1)?;
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.switch_tab_relative(-1)?;
            }
            KeyCode::Char('/') => self.open_finder(FinderMode::Files)?,
            KeyCode::Char('F')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && key.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                self.open_finder(FinderMode::Content)?;
            }
            KeyCode::Char('f')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && key.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                self.open_finder(FinderMode::Content)?;
            }
            KeyCode::Char('h') | KeyCode::Left => self.move_cursor(MoveDir::Left),
            KeyCode::Char('j') | KeyCode::Down => self.move_cursor(MoveDir::Down),
            KeyCode::Char('k') | KeyCode::Up => self.move_cursor(MoveDir::Up),
            KeyCode::Char('l') | KeyCode::Right => self.move_cursor(MoveDir::Right),
            KeyCode::Char('0') => self.move_cursor(MoveDir::LineStart),
            KeyCode::Char('$') => self.move_cursor(MoveDir::LineEnd),
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.save_buffer()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_key_command(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.command_input.clear();
                self.mark_render_dirty();
            }
            KeyCode::Enter => {
                let command = self.command_input.trim().to_string();
                self.mode = Mode::Normal;
                self.command_input.clear();
                self.mark_render_dirty();

                if !command.is_empty() {
                    let _ = self.event_tx.send(Msg::PluginCommand(command));
                }
            }
            KeyCode::Backspace => {
                self.command_input.pop();
                self.mark_render_dirty();
            }
            KeyCode::Char(ch)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.command_input.push(ch);
                self.mark_render_dirty();
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_key_insert(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('e') {
            self.sidebar_visible = !self.sidebar_visible;
            if self.sidebar_visible {
                self.file_tree.refresh()?;
                self.mode = Mode::Sidebar;
            }
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Enter => {
                self.buffer.insert_newline();
                self.mark_render_dirty();
                self.schedule_auto_save();
            }
            KeyCode::Backspace => {
                self.buffer.delete_char_before();
                self.mark_render_dirty();
                self.schedule_auto_save();
            }
            KeyCode::Char(ch) => {
                self.buffer.insert_char(ch);
                self.mark_render_dirty();
                self.schedule_auto_save();
            }
            KeyCode::Left => self.move_cursor(MoveDir::Left),
            KeyCode::Right => self.move_cursor(MoveDir::Right),
            KeyCode::Up => self.move_cursor(MoveDir::Up),
            KeyCode::Down => self.move_cursor(MoveDir::Down),
            _ => {}
        }
        Ok(())
    }

    fn handle_key_sidebar(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('e') {
            self.sidebar_visible = false;
            self.mode = Mode::Normal;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
            }
            KeyCode::Char('j') | KeyCode::Down => self.file_tree.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.file_tree.move_selection(-1),
            KeyCode::Char('h') | KeyCode::Left => self.file_tree.collapse_selected_or_parent()?,
            KeyCode::Char('l') | KeyCode::Right => {
                if let Some(node) = self.file_tree.selected_node() {
                    if node.is_dir {
                        self.file_tree.toggle_selected_dir()?;
                    } else {
                        self.open_file(node.path.clone())?;
                        self.mode = Mode::Normal;
                    }
                }
            }
            KeyCode::Char('a') => {
                self.file_tree.begin_create();
                self.mode = Mode::SidebarCreate;
            }
            KeyCode::Enter => {
                if let Some(node) = self.file_tree.selected_node() {
                    if node.is_dir {
                        self.file_tree.toggle_selected_dir()?;
                    } else {
                        self.open_file(node.path.clone())?;
                        self.mode = Mode::Normal;
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_key_confirm_create(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.confirm_create_wikilink()?;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.pending_create_path = None;
                self.mode = Mode::Normal;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_key_backlinks(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('b') {
            self.toggle_backlinks_panel()?;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.backlinks_visible = false;
                self.mode = Mode::Normal;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.backlinks.is_empty() {
                    self.backlinks_selected =
                        (self.backlinks_selected + 1).min(self.backlinks.len().saturating_sub(1));
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.backlinks_selected = self.backlinks_selected.saturating_sub(1);
            }
            KeyCode::Enter => {
                if let Some(entry) = self.backlinks.get(self.backlinks_selected).cloned() {
                    self.open_file(entry.path)?;
                    let target = entry.line.saturating_sub(1);
                    self.buffer.cursor.row = target.min(self.buffer.line_count().saturating_sub(1));
                    self.buffer.cursor.col = 0;
                    self.buffer.cursor.desired_col = 0;
                    self.buffer.scroll_to_cursor();
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_key_sidebar_create(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.file_tree.create_input.clear();
                self.mode = Mode::Sidebar;
            }
            KeyCode::Backspace => {
                self.file_tree.create_input.pop();
            }
            KeyCode::Enter => {
                if let Some(path) = self.file_tree.commit_create()? {
                    self.open_file(path)?;
                    self.mode = Mode::Normal;
                } else {
                    self.mode = Mode::Sidebar;
                }
            }
            KeyCode::Char(ch) => {
                self.file_tree.create_input.push(ch);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_key_finder(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.finder_query.clear();
                self.finder_results.clear();
                self.finder_selected = 0;
            }
            KeyCode::Enter => {
                if let Some(result) = self.finder_results.get(self.finder_selected).cloned() {
                    self.open_file(result.path)?;
                    if let Some(line) = result.line {
                        let target = line.saturating_sub(1);
                        self.buffer.cursor.row =
                            target.min(self.buffer.line_count().saturating_sub(1));
                        self.buffer.cursor.col = 0;
                        self.buffer.cursor.desired_col = 0;
                        self.buffer.scroll_to_cursor();
                    }
                }
                self.mode = Mode::Normal;
                self.finder_query.clear();
                self.finder_results.clear();
                self.finder_selected = 0;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.finder_results.is_empty() {
                    self.finder_selected =
                        (self.finder_selected + 1).min(self.finder_results.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if !self.finder_results.is_empty() {
                    self.finder_selected = self.finder_selected.saturating_sub(1);
                }
            }
            KeyCode::Backspace => {
                self.finder_query.pop();
                self.refresh_finder_results()?;
            }
            KeyCode::Char(ch)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.finder_query.push(ch);
                self.refresh_finder_results()?;
            }
            _ => {}
        }

        Ok(())
    }

    fn move_cursor(&mut self, dir: MoveDir) {
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

    fn schedule_auto_save(&mut self) {
        let debounce_ms = self.config.general.auto_save_debounce_ms;
        self.buffer.save_debounce = Some(Instant::now() + Duration::from_millis(debounce_ms));
    }

    fn handle_tick(&mut self) -> Result<()> {
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

    fn save_buffer(&mut self) -> Result<()> {
        let Some(path) = self.buffer.path.clone() else {
            return Ok(());
        };

        self.save_active_buffer_at_path(path);

        Ok(())
    }

    fn save_active_buffer_at_path(&mut self, path: PathBuf) {
        self.buffer.save_debounce = None;
        self.buffer.dirty = false;
        self.last_saved_file = Some((path.clone(), Instant::now()));

        let rope = self.buffer.rope.clone();
        spawn_buffer_save(path, rope);
    }

    fn save_inactive_buffer(&mut self, path: &PathBuf) {
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

    fn save_all_buffers(&mut self) {
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

    fn handle_file_changed(&mut self, path: PathBuf) -> Result<()> {
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

    fn should_reload_active(&self, path: &PathBuf) -> bool {
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

    fn open_file(&mut self, path: PathBuf) -> Result<()> {
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

    fn mark_render_dirty(&mut self) {
        self.render_cache.dirty = true;
    }

    fn switch_tab_relative(&mut self, delta: isize) -> Result<()> {
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

    fn active_tab_index(&self) -> Option<usize> {
        let active = self.buffer.path.as_ref()?;
        self.open_tabs
            .iter()
            .position(|p| same_file_path(p, active))
    }

    fn activate_tab(&mut self, path: PathBuf) -> Result<()> {
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

    fn toggle_backlinks_panel(&mut self) -> Result<()> {
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

    fn refresh_backlinks(&mut self) {
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

    fn follow_wikilink_under_cursor(&mut self) -> Result<()> {
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

    fn confirm_create_wikilink(&mut self) -> Result<()> {
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

    fn wikilink_under_cursor(&self) -> Option<String> {
        let line = self.buffer.line_text(self.buffer.cursor.row)?;
        let col = self.buffer.cursor.col;

        for m in WIKILINK_RE.find_iter(&line) {
            if col >= m.start() && col < m.end() {
                return parse_wikilink_target(&line[m.start()..m.end()]);
            }
        }

        None
    }

    fn resolve_wikilink_target(&self, link_text: &str) -> Option<PathBuf> {
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

    fn open_finder(&mut self, mode: FinderMode) -> Result<()> {
        self.mode = Mode::FinderOpen;
        self.finder_mode = mode;
        self.finder_query.clear();
        self.finder_selected = 0;
        self.file_tree.refresh()?;
        self.refresh_finder_results()
    }

    fn refresh_finder_results(&mut self) -> Result<()> {
        self.file_tree.refresh()?;

        let files = self.file_tree.all_file_paths();
        let limit = self.config.search.max_results;

        self.finder_results.clear();

        if self.finder_mode == FinderMode::Files {
            if self.finder_query.is_empty() {
                self.finder_results = files
                    .into_iter()
                    .take(limit)
                    .map(|path| FinderResult {
                        preview: path.to_string_lossy().to_string(),
                        path,
                        line: None,
                    })
                    .collect();
                self.finder_selected = 0;
                return Ok(());
            }

            let matcher = SkimMatcherV2::default();
            let mut scored: Vec<(i64, FinderResult)> = files
                .into_iter()
                .filter_map(|path| {
                    let candidate = path.to_string_lossy().to_string();
                    matcher
                        .fuzzy_match(&candidate, &self.finder_query)
                        .map(|score| {
                            (
                                score,
                                FinderResult {
                                    path,
                                    line: None,
                                    preview: candidate,
                                },
                            )
                        })
                })
                .collect();

            scored.sort_by(|a, b| b.0.cmp(&a.0));

            self.finder_results = scored
                .into_iter()
                .take(limit)
                .map(|(_, item)| item)
                .collect();
        } else {
            if self.finder_query.is_empty() {
                self.finder_selected = 0;
                return Ok(());
            }

            let needle = self.finder_query.to_lowercase();
            let mut hits = Vec::new();

            for path in files {
                let Ok(contents) = std::fs::read_to_string(&path) else {
                    continue;
                };

                for (idx, line) in contents.lines().enumerate() {
                    if line.to_lowercase().contains(&needle) {
                        hits.push(FinderResult {
                            preview: format!(
                                "{}:{}  {}",
                                path.to_string_lossy(),
                                idx + 1,
                                line.trim()
                            ),
                            path: path.clone(),
                            line: Some(idx + 1),
                        });
                        if hits.len() >= limit {
                            break;
                        }
                    }
                }

                if hits.len() >= limit {
                    break;
                }
            }

            self.finder_results = hits;
        }

        if self.finder_results.is_empty() {
            self.finder_selected = 0;
        } else if self.finder_selected >= self.finder_results.len() {
            self.finder_selected = self.finder_results.len() - 1;
        }

        Ok(())
    }

    // ── MVU: View ────────────────────────────────────────────────

    pub fn view(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // tab bar
                Constraint::Min(1),    // body
                Constraint::Length(1), // status bar
            ])
            .split(frame.area());

        self.render_tab_bar(frame, chunks[0]);

        let editor_area = if self.sidebar_visible && self.backlinks_visible {
            let body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(22),
                    Constraint::Min(1),
                    Constraint::Percentage(25),
                ])
                .split(chunks[1]);

            self.render_sidebar(frame, body[0]);
            self.render_editor(frame, body[1]);
            self.render_backlinks_panel(frame, body[2]);
            body[1]
        } else if self.sidebar_visible {
            let body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(22), Constraint::Min(1)])
                .split(chunks[1]);

            self.render_sidebar(frame, body[0]);
            self.render_editor(frame, body[1]);
            body[1]
        } else if self.backlinks_visible {
            let body = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(1), Constraint::Percentage(25)])
                .split(chunks[1]);

            self.render_editor(frame, body[0]);
            self.render_backlinks_panel(frame, body[1]);
            body[0]
        } else {
            self.render_editor(frame, chunks[1]);
            chunks[1]
        };

        let cursor_x = self.buffer.cursor.col as u16 + editor_area.x;
        let cursor_y =
            (self.buffer.cursor.row - self.buffer.viewport.top_line) as u16 + editor_area.y;
        if cursor_y < editor_area.y + editor_area.height {
            frame.set_cursor_position((cursor_x, cursor_y));
        }

        self.render_status_bar(frame, chunks[2]);

        if self.mode == Mode::FinderOpen {
            self.render_finder_overlay(frame);
        } else if self.mode == Mode::Command {
            self.render_command_overlay(frame);
        }
    }

    fn render_editor(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let top = self.buffer.viewport.top_line;
        let bottom = (top + area.height as usize).min(self.buffer.line_count());

        let needs_rebuild = self.render_cache.dirty
            || self.render_cache.top != top
            || self.render_cache.bottom != bottom;

        if needs_rebuild {
            let mut code_block_lang = self.code_block_lang_before_line(top);
            self.render_cache.lines = (top..bottom)
                .map(|i| {
                    let text = self.buffer.line_text(i).unwrap_or_default();
                    self.render_markdown_line(&text, &mut code_block_lang)
                })
                .collect();
            self.render_cache.top = top;
            self.render_cache.bottom = bottom;
            self.render_cache.dirty = false;
        }

        let editor = Paragraph::new(self.render_cache.lines.clone());
        frame.render_widget(editor, area);
    }

    fn code_block_lang_before_line(&self, line_index: usize) -> Option<String> {
        if line_index == 0 {
            return None;
        }

        let mut code_block_lang = None;
        for i in 0..line_index {
            let text = self.buffer.line_text(i).unwrap_or_default();
            if let Some(lang) = parse_code_fence_language(&text) {
                if code_block_lang.is_some() {
                    code_block_lang = None;
                } else {
                    code_block_lang = Some(lang);
                }
            }
        }
        code_block_lang
    }

    fn render_markdown_line(
        &self,
        text: &str,
        code_block_lang: &mut Option<String>,
    ) -> Line<'static> {
        if let Some(lang) = parse_code_fence_language(text) {
            if code_block_lang.is_some() {
                *code_block_lang = None;
            } else {
                *code_block_lang = Some(lang);
            }
            return Line::from(Span::styled(
                text.to_string(),
                Style::default()
                    .fg(Color::Rgb(180, 180, 200))
                    .bg(Color::Rgb(25, 25, 42))
                    .add_modifier(Modifier::BOLD),
            ));
        }

        if let Some(lang) = code_block_lang.as_deref() {
            return self.render_code_block_line(text, lang);
        }

        let base_style = self.base_markdown_style(text);
        self.render_inline_markdown(text, base_style)
    }

    fn render_code_block_line(&self, text: &str, language: &str) -> Line<'static> {
        let syntax = SYNTAX_SET
            .find_syntax_by_token(language)
            .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());
        let mut highlighter = HighlightLines::new(syntax, &SYNTECT_THEME);

        let highlighted = highlighter.highlight_line(text, &SYNTAX_SET);
        let Ok(tokens) = highlighted else {
            return Line::from(Span::styled(
                text.to_string(),
                Style::default()
                    .fg(Color::Rgb(200, 200, 200))
                    .bg(Color::Rgb(18, 18, 28)),
            ));
        };

        let spans: Vec<Span<'static>> = tokens
            .into_iter()
            .map(|(style, segment)| Span::styled(segment.to_string(), syntect_to_ratatui(style)))
            .collect();

        if spans.is_empty() {
            Line::from(Span::styled(
                text.to_string(),
                Style::default()
                    .fg(Color::Rgb(200, 200, 200))
                    .bg(Color::Rgb(18, 18, 28)),
            ))
        } else {
            Line::from(spans)
        }
    }

    fn base_markdown_style(&self, text: &str) -> Style {
        let trimmed = text.trim_start();

        if trimmed.starts_with("# ") {
            return Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD);
        }
        if trimmed.starts_with("## ") {
            return Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD);
        }
        if trimmed.starts_with("### ") {
            return Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD);
        }
        if trimmed.starts_with(">") {
            return Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC);
        }
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
            return Style::default().fg(Color::LightCyan);
        }

        Style::default().fg(Color::Gray)
    }

    fn render_inline_markdown(&self, text: &str, base_style: Style) -> Line<'static> {
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut cursor = 0;

        while cursor < text.len() {
            let next = next_markdown_token(text, cursor);

            let Some((start, end, kind)) = next else {
                if cursor < text.len() {
                    spans.push(Span::styled(text[cursor..].to_string(), base_style));
                }
                break;
            };

            if start > cursor {
                spans.push(Span::styled(text[cursor..start].to_string(), base_style));
            }

            let token_style = match kind {
                TokenKind::WikiLink => base_style
                    .fg(Color::Rgb(0, 255, 136))
                    .add_modifier(Modifier::UNDERLINED | Modifier::BOLD),
                TokenKind::Link => base_style
                    .fg(Color::Rgb(255, 102, 0))
                    .add_modifier(Modifier::UNDERLINED),
                TokenKind::InlineCode => base_style
                    .fg(Color::Rgb(220, 220, 220))
                    .bg(Color::Rgb(32, 32, 48)),
                TokenKind::Bold => base_style.add_modifier(Modifier::BOLD),
                TokenKind::Italic => base_style.add_modifier(Modifier::ITALIC),
            };

            spans.push(Span::styled(text[start..end].to_string(), token_style));
            cursor = end;
        }

        if spans.is_empty() {
            Line::from(Span::styled(text.to_string(), base_style))
        } else {
            Line::from(spans)
        }
    }

    fn render_status_bar(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let mode_style = match self.mode {
            Mode::Normal => Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
            Mode::Insert => Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            _ => Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        };

        let mode_span = Span::styled(format!(" {} ", self.mode.label()), mode_style);

        let file_name = self
            .buffer
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "[scratch]".to_string());

        let dirty_marker = if self.buffer.dirty { " [+]" } else { "" };

        let mut suffix = match self.mode {
            Mode::SidebarCreate => format!(" | new: {}", self.file_tree.create_input),
            Mode::FinderOpen => {
                let label = if self.finder_mode == FinderMode::Files {
                    "find"
                } else {
                    "search"
                };
                format!(" | {label}: {}", self.finder_query)
            }
            Mode::Command => format!(" | :{}", self.command_input),
            Mode::ConfirmCreate => {
                if let Some(path) = &self.pending_create_path {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "note.md".to_string());
                    format!(" | create {name}? (y/n)")
                } else {
                    " | create note? (y/n)".to_string()
                }
            }
            _ => String::new(),
        };

        if self.quit_confirm_armed {
            let pending = self.pending_write_count();
            suffix.push_str(&format!(" | {pending} pending, press q again to save+quit"));
        }

        let info = Span::styled(
            format!(
                " {file_name}{dirty_marker}  {}:{}{} ",
                self.buffer.cursor.row + 1,
                self.buffer.cursor.col + 1,
                suffix
            ),
            Style::default().fg(Color::Gray).bg(Color::DarkGray),
        );

        let bar = Line::from(vec![mode_span, info]);
        let status = Paragraph::new(bar).style(Style::default().bg(Color::DarkGray));
        frame.render_widget(status, area);
    }

    fn render_tab_bar(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let active_path = self.buffer.path.as_ref();
        let mut spans = Vec::new();

        for tab_path in &self.open_tabs {
            let name = tab_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "[note]".to_string());

            let is_active = active_path.is_some_and(|p| same_file_path(p, tab_path));
            let mut label = format!(" {name} ");
            if is_active && self.buffer.dirty {
                label = format!(" {name} ● ");
            }

            let style = if is_active {
                Style::default()
                    .bg(Color::Rgb(30, 30, 45))
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().bg(Color::Rgb(18, 18, 28)).fg(Color::Gray)
            };

            spans.push(Span::styled(label, style));
        }

        spans.push(Span::styled(
            "  Ctrl+N/P: Tabs  Ctrl+E: Sidebar  Ctrl+B: Backlinks  gd: Follow Link  /: Finder  Ctrl+Shift+F: Search  : Command (help)  i: Insert  q: Confirm Quit  Q: Save+Quit ",
            Style::default()
                .bg(Color::Rgb(20, 20, 30))
                .fg(Color::DarkGray),
        ));

        let line = Line::from(spans);

        frame.render_widget(
            Paragraph::new(line).style(Style::default().bg(Color::Rgb(20, 20, 30))),
            area,
        );
    }

    fn render_sidebar(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let lines: Vec<Line> = self
            .file_tree
            .nodes
            .iter()
            .enumerate()
            .map(|(idx, node)| {
                let indent = "  ".repeat(node.depth);
                let prefix = if node.is_dir {
                    if self.file_tree.is_expanded(&node.path) {
                        "▾ "
                    } else {
                        "▸ "
                    }
                } else {
                    "  "
                };
                let content = format!("{indent}{prefix}{}", node.name);

                if idx == self.file_tree.selected {
                    Line::from(Span::styled(
                        content,
                        Style::default().fg(Color::Black).bg(Color::Cyan),
                    ))
                } else {
                    Line::from(Span::styled(content, Style::default().fg(Color::Gray)))
                }
            })
            .collect();

        frame.render_widget(
            Paragraph::new(lines).style(Style::default().bg(Color::Rgb(12, 12, 18))),
            area,
        );
    }

    fn render_backlinks_panel(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let lines: Vec<Line> = if self.backlinks.is_empty() {
            vec![Line::from(Span::styled(
                "No backlinks",
                Style::default().fg(Color::DarkGray),
            ))]
        } else {
            self.backlinks
                .iter()
                .enumerate()
                .map(|(idx, entry)| {
                    let file = entry
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "[note]".to_string());
                    let mut preview = entry.preview.clone();
                    if preview.len() > 42 {
                        preview.truncate(42);
                        preview.push('…');
                    }

                    let label = format!("{file}:{}  {preview}", entry.line);
                    if idx == self.backlinks_selected {
                        Line::from(Span::styled(
                            label,
                            Style::default().fg(Color::Black).bg(Color::Cyan),
                        ))
                    } else {
                        Line::from(Span::styled(label, Style::default().fg(Color::Gray)))
                    }
                })
                .collect()
        };

        let panel = Paragraph::new(lines).block(
            Block::default()
                .title(" Backlinks ")
                .borders(Borders::LEFT)
                .style(Style::default().bg(Color::Rgb(12, 12, 18))),
        );
        frame.render_widget(panel, area);
    }

    fn render_finder_overlay(&self, frame: &mut Frame) {
        let area = centered_rect(70, 60, frame.area());
        frame.render_widget(Clear, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(area);

        let input = Paragraph::new(self.finder_query.clone()).block(
            Block::default()
                .title(if self.finder_mode == FinderMode::Files {
                    " Finder (Files) "
                } else {
                    " Search (Content) "
                })
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Rgb(15, 15, 24))),
        );
        frame.render_widget(input, chunks[0]);

        let results: Vec<Line> = if self.finder_results.is_empty() {
            vec![Line::from(Span::styled(
                "No matches",
                Style::default().fg(Color::DarkGray),
            ))]
        } else {
            self.finder_results
                .iter()
                .enumerate()
                .map(|(idx, item)| {
                    let label = item.preview.clone();
                    if idx == self.finder_selected {
                        Line::from(Span::styled(
                            format!("> {label}"),
                            Style::default().fg(Color::Black).bg(Color::Cyan),
                        ))
                    } else {
                        Line::from(Span::styled(
                            format!("  {label}"),
                            Style::default().fg(Color::Gray),
                        ))
                    }
                })
                .collect()
        };

        let result_block = Paragraph::new(results).block(
            Block::default()
                .borders(Borders::LEFT | Borders::RIGHT)
                .style(Style::default().bg(Color::Rgb(10, 10, 18))),
        );
        frame.render_widget(result_block, chunks[1]);

        let footer = Paragraph::new(" Enter: open  Esc: close  j/k: move ").block(
            Block::default().borders(Borders::ALL).style(
                Style::default()
                    .bg(Color::Rgb(15, 15, 24))
                    .fg(Color::DarkGray),
            ),
        );
        frame.render_widget(footer, chunks[2]);

        let cursor_x = chunks[0].x + 1 + self.finder_query.len() as u16;
        let cursor_y = chunks[0].y + 1;
        frame.set_cursor_position((cursor_x, cursor_y));
    }

    fn render_command_overlay(&self, frame: &mut Frame) {
        let area = centered_rect(70, 20, frame.area());
        frame.render_widget(Clear, area);

        let prompt = Paragraph::new(format!(":{}", self.command_input)).block(
            Block::default()
                .title(" Command ")
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Rgb(15, 15, 24))),
        );
        frame.render_widget(prompt, area);

        let cursor_x = area.x + 2 + self.command_input.len() as u16;
        let cursor_y = area.y + 1;
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn parse_plugin_command_input(raw: &str) -> String {
    let input = raw.trim();
    if input.len() < 2 {
        return input.to_string();
    }

    let mut chars = input.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };

    if first != '"' && first != '\'' {
        return input.to_string();
    }

    if !input.ends_with(first) {
        return input.to_string();
    }

    let inner = &input[first.len_utf8()..input.len() - first.len_utf8()];
    let mut out = String::with_capacity(inner.len());
    let mut escaped = false;

    for ch in inner.chars() {
        if escaped {
            out.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else {
            out.push(ch);
        }
    }

    if escaped {
        out.push('\\');
    }

    out.trim().to_string()
}

fn same_file_path(a: &PathBuf, b: &PathBuf) -> bool {
    if a == b {
        return true;
    }

    let a_canon = std::fs::canonicalize(a);
    let b_canon = std::fs::canonicalize(b);
    matches!((a_canon, b_canon), (Ok(ca), Ok(cb)) if ca == cb)
}

fn spawn_buffer_save(path: PathBuf, rope: ropey::Rope) {
    std::thread::spawn(move || {
        use std::io::Write;
        let result = (|| -> Result<()> {
            let tmp = path.with_extension("tmp");
            let file = std::fs::File::create(&tmp)?;
            let mut writer = std::io::BufWriter::new(file);
            for chunk in rope.chunks() {
                writer.write_all(chunk.as_bytes())?;
            }
            writer.flush()?;
            std::fs::rename(&tmp, &path)?;
            Ok(())
        })();

        if let Err(e) = result {
            tracing::error!("save failed: {e}");
        }
    });
}

#[derive(Debug, Clone, Copy)]
enum TokenKind {
    WikiLink,
    Link,
    InlineCode,
    Bold,
    Italic,
}

fn next_markdown_token(text: &str, start_at: usize) -> Option<(usize, usize, TokenKind)> {
    let candidates = [
        (
            INLINE_CODE_RE
                .find_at(text, start_at)
                .map(|m| (m.start(), m.end(), TokenKind::InlineCode)),
            0,
        ),
        (
            WIKILINK_RE
                .find_at(text, start_at)
                .map(|m| (m.start(), m.end(), TokenKind::WikiLink)),
            1,
        ),
        (
            LINK_RE
                .find_at(text, start_at)
                .map(|m| (m.start(), m.end(), TokenKind::Link)),
            2,
        ),
        (
            BOLD_RE
                .find_at(text, start_at)
                .map(|m| (m.start(), m.end(), TokenKind::Bold)),
            3,
        ),
        (
            ITALIC_RE
                .find_at(text, start_at)
                .map(|m| (m.start(), m.end(), TokenKind::Italic)),
            4,
        ),
    ];

    candidates
        .into_iter()
        .filter_map(|(hit, priority)| hit.map(|h| (h, priority)))
        .min_by(|((sa, _, _), pa), ((sb, _, _), pb)| sa.cmp(sb).then(pa.cmp(pb)))
        .map(|(h, _)| h)
}

fn parse_code_fence_language(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with("```") {
        return None;
    }

    let lang = trimmed.trim_start_matches("```").trim();
    if lang.is_empty() {
        Some("text".to_string())
    } else {
        Some(lang.to_string())
    }
}

fn parse_wikilink_target(wikilink: &str) -> Option<String> {
    if !(wikilink.starts_with("[[") && wikilink.ends_with("]]")) {
        return None;
    }

    let inner = &wikilink[2..wikilink.len().saturating_sub(2)];
    let sanitized = sanitize_link_name(inner);
    if sanitized.is_empty() {
        None
    } else {
        Some(sanitized)
    }
}

fn sanitize_link_name(raw: &str) -> String {
    raw.split(['|', '#'])
        .next()
        .map(str::trim)
        .unwrap_or_default()
        .to_string()
}

fn syntect_to_ratatui(style: syntect::highlighting::Style) -> Style {
    let mut rat_style = Style::default()
        .fg(Color::Rgb(
            style.foreground.r,
            style.foreground.g,
            style.foreground.b,
        ))
        .bg(Color::Rgb(
            style.background.r,
            style.background.g,
            style.background.b,
        ));

    if style.font_style.contains(FontStyle::BOLD) {
        rat_style = rat_style.add_modifier(Modifier::BOLD);
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        rat_style = rat_style.add_modifier(Modifier::ITALIC);
    }
    if style.font_style.contains(FontStyle::UNDERLINE) {
        rat_style = rat_style.add_modifier(Modifier::UNDERLINED);
    }

    rat_style
}
