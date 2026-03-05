use std::collections::HashMap;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::mpsc;
use std::time::Instant;

use anyhow::Result;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use regex::Regex;
use syntect::highlighting::{FontStyle, Theme as SyntectTheme, ThemeSet};
use syntect::parsing::SyntaxSet;

use crate::model::buffer::Buffer;
use crate::model::config::AppConfig;
use crate::model::file_tree::FileTree;
use crate::model::mode::Mode;
use crate::msg::Msg;
use crate::plugin::PluginManager;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FinderMode {
    Files,
    Content,
}

#[derive(Debug, Clone)]
pub(crate) struct FinderResult {
    pub(crate) path: PathBuf,
    pub(crate) line: Option<usize>,
    pub(crate) preview: String,
}

#[derive(Debug, Clone)]
pub(crate) struct BacklinkEntry {
    pub(crate) path: PathBuf,
    pub(crate) line: usize,
    pub(crate) preview: String,
}

#[derive(Default)]
pub(crate) struct RenderCache {
    pub(crate) top: usize,
    pub(crate) bottom: usize,
    pub(crate) lines: Vec<Line<'static>>,
    pub(crate) dirty: bool,
}

pub(crate) static WIKILINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[[^\]]+\]\]").expect("valid wikilink regex"));
pub(crate) static LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[[^\]]+\]\([^\)]+\)").expect("valid markdown link regex"));
pub(crate) static INLINE_CODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"`[^`]+`").expect("valid inline code regex"));
pub(crate) static BOLD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\*\*[^*]+\*\*").expect("valid bold regex"));
pub(crate) static ITALIC_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\*[^*\s][^*]*\*").expect("valid italic regex"));
pub(crate) static SYNTAX_SET: LazyLock<SyntaxSet> =
    LazyLock::new(SyntaxSet::load_defaults_newlines);
pub(crate) static SYNTECT_THEME: LazyLock<SyntectTheme> = LazyLock::new(|| {
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
    pub(crate) inactive_buffers: HashMap<PathBuf, Buffer>,
    pub(crate) open_tabs: Vec<PathBuf>,
    pub file_tree: FileTree,
    pub sidebar_visible: bool,
    pub(crate) finder_mode: FinderMode,
    pub(crate) finder_query: String,
    pub(crate) finder_results: Vec<FinderResult>,
    pub(crate) finder_selected: usize,
    pub(crate) command_input: String,
    pub config: AppConfig,
    #[allow(dead_code)]
    // Phase 3 scaffolding: runtime command/event dispatch will read this manager.
    pub plugin_manager: PluginManager,
    pub should_quit: bool,
    #[allow(dead_code)] // Phase 2: plugin system event bus
    pub event_tx: mpsc::Sender<Msg>,
    #[allow(dead_code)] // Phase 2: status bar notifications
    pub notifications: VecDeque<String>,
    pub(crate) render_cache: RenderCache,
    pub(crate) last_saved_file: Option<(PathBuf, Instant)>,
    pub(crate) quit_confirm_armed: bool,
    pub(crate) quit_confirm_until: Option<Instant>,
    pub(crate) pending_key: Option<char>,
    pub(crate) pending_create_path: Option<PathBuf>,
    pub(crate) backlinks_visible: bool,
    pub(crate) backlinks: Vec<BacklinkEntry>,
    pub(crate) backlinks_selected: usize,
    #[allow(dead_code)] // Phase 2: animation tick tracking
    pub(crate) last_tick: Instant,
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

    pub(crate) fn pending_write_count(&self) -> usize {
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

    pub(crate) fn mark_render_dirty(&mut self) {
        self.render_cache.dirty = true;
    }

    // ── MVU: View ────────────────────────────────────────────────
}

pub(crate) fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

pub(crate) fn parse_plugin_command_input(raw: &str) -> String {
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

pub(crate) fn same_file_path(a: &PathBuf, b: &PathBuf) -> bool {
    if a == b {
        return true;
    }

    let a_canon = std::fs::canonicalize(a);
    let b_canon = std::fs::canonicalize(b);
    matches!((a_canon, b_canon), (Ok(ca), Ok(cb)) if ca == cb)
}

pub(crate) fn spawn_buffer_save(path: PathBuf, rope: ropey::Rope) {
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
pub(crate) enum TokenKind {
    WikiLink,
    Link,
    InlineCode,
    Bold,
    Italic,
}

pub(crate) fn next_markdown_token(
    text: &str,
    start_at: usize,
) -> Option<(usize, usize, TokenKind)> {
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

pub(crate) fn parse_code_fence_language(line: &str) -> Option<String> {
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

pub(crate) fn parse_wikilink_target(wikilink: &str) -> Option<String> {
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

pub(crate) fn sanitize_link_name(raw: &str) -> String {
    raw.split(['|', '#'])
        .next()
        .map(str::trim)
        .unwrap_or_default()
        .to_string()
}

pub(crate) fn syntect_to_ratatui(style: syntect::highlighting::Style) -> Style {
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
