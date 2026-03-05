# BlackBox — Technical Architecture & Execution Roadmap

> **Version:** 0.2.0
> **Author:** Senior AI Architect / Lead Rust Engineer
> **Date:** 2026-03-05
> **Status:** CURRENT

---

## 0. Design Principles

| Principle                     | Constraint                                                                                                                          |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| **Local First**               | The file system _is_ the database. No SQLite, no RocksDB. `~/.blackbox/` is a plain directory of `.md` files.                       |
| **Sub-100ms Startup**         | No async runtime boot. No lazy plugin loading on the hot path. Cold start → rendered frame in <100ms on an M1.                      |
| **Zero-Copy Where It Counts** | Markdown parsing, search, and rendering operate on `&str` borrows from a rope buffer—never clone the full document.                 |
| **Elm/MVU Strict**            | All mutations flow through `Msg → update() → Model`. The `view()` function is pure: `&Model → Frame`. No side-effects in rendering. |

---

## 1. System Architecture

### 1.1 Application State (`Model`)

```rust
/// Root application state. Single source of truth.
pub struct App {
    pub mode: Mode,
    pub buffers: BufferManager,
    pub file_tree: FileTree,
    pub finder: FuzzyFinder,
    pub config: AppConfig,
    pub notifications: VecDeque<Notification>,
    pub should_quit: bool,
    pub last_tick: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Command,            // `:` command palette
    FinderOpen,         // fuzzy file picker overlay
    LinkPicker,         // [[WikiLink]] autocomplete
    PluginPane(PluginId),
}

pub struct BufferManager {
    pub active: BufferId,
    pub scratch: BufferId,           // the "Never-Lost" inbox, always index 0
    buffers: SlotMap<BufferId, Buffer>,
}

pub struct Buffer {
    pub id: BufferId,
    pub rope: Rope,                  // `ropey::Rope` — the backing text store
    pub path: Option<PathBuf>,       // None = scratch / unsaved
    pub dirty: bool,
    pub cursor: CursorState,
    pub viewport: Viewport,
    pub highlights: HighlightCache,  // pre-computed styled spans per visible line
    pub save_debounce: Option<Instant>,
    pub undo_tree: UndoTree,
}

pub struct CursorState {
    pub row: usize,
    pub col: usize,
    pub desired_col: usize,         // "sticky" column for vertical movement
    pub selection: Option<(Position, Position)>,
}

pub struct Viewport {
    pub top_line: usize,
    pub height: u16,
    pub scroll_off: u16,            // lines of context kept above/below cursor
}

pub struct HighlightCache {
    spans: Vec<Vec<(Style, Range<usize>)>>,  // one per visible line
    valid_range: Range<usize>,                // lines that are up-to-date
}
```

**Key decisions:**

- **`ropey::Rope`** for the text buffer — `O(log n)` inserts/deletes, efficient line indexing, and `Chunks` iterator for zero-copy access. No `String` allocation per edit.
- **`SlotMap<BufferId, Buffer>`** for buffer storage — `O(1)` lookup with stable generational IDs. Avoids `Vec` index invalidation when buffers close.
- **`HighlightCache`** is invalidated lazily per-line: an edit on line 14 only re-highlights lines 14..N where N is the end of the affected Markdown block (e.g., a fenced code block). The `valid_range` tracks the contiguous region that doesn't need recomputation.

### 1.2 Message Type (`Msg`)

```rust
pub enum Msg {
    // -- Input events
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),

    // -- Buffer operations
    InsertChar(char),
    DeleteChar,
    NewLine,
    MoveCursor(Direction),
    SetMode(Mode),

    // -- File I/O
    SaveBuffer(BufferId),
    OpenFile(PathBuf),
    FileChanged(PathBuf),       // from `notify` watcher
    ScratchAutoSave,

    // -- Search / Navigation
    FuzzyQuery(String),
    FuzzySelect(usize),
    FollowWikiLink(String),

    // -- Plugin
    PluginEvent(PluginId, PluginMsg),

    // -- System
    Tick,                        // periodic 50ms tick for debounce/animations
    Quit,
}
```

### 1.3 The Event Loop

```
┌────────────────────────────────────────────────────────────────────┐
│                        MAIN THREAD                                 │
│                                                                    │
│   ┌────────────┐    ┌──────────────┐    ┌───────────┐              │
│   │ crossterm  │───▶│  Event       │───▶│  update() │              │
│   │ EventStream│    │  Aggregator  │    │  (MVU)    │              │
│   └────────────┘    │  (channel)   │    └─────┬─────┘              │
│                     │              │          │                    │
│   ┌──────────┐      │              │          ▼                    │
│   │ notify   │─────▶│              │    ┌───────────┐              │
│   │ watcher  │      │              │    │  view()   │──▶ Terminal  │
│   └──────────┘      │              │    │  (render) │              │
│                     │              │    └───────────┘              │
│   ┌──────────┐      │              │                               │
│   │ tick     │─────▶│              │                               │
│   │ timer    │      └──────────────┘                               │
│   └──────────┘                                                     │
│                                                                    │
│   ┌──────────────────────────────────────────────────┐             │
│   │               BACKGROUND THREAD POOL             │             │
│   │  ┌─────────┐ ┌─────────┐ ┌──────────────────┐    │             │
│   │  │  File   │ │ Search  │ │  Plugin WASM     │    │             │
│   │  │  I/O    │ │ indexer │ │  sandbox         │    │             │
│   │  └─────────┘ └─────────┘ └──────────────────┘    │             │
│   └───────────────────┬──────────────────────────────┘             │
│                       │ results via channel                        │
│                       ▼                                            │
│                   Event Aggregator                                 │
└────────────────────────────────────────────────────────────────────┘
```

**Implementation:**

```rust
fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    let (tx, rx) = mpsc::channel::<Msg>();
    let mut app = App::new()?;

    // Spawn input thread
    let tx_input = tx.clone();
    thread::spawn(move || {
        loop {
            if let Ok(event) = crossterm::event::read() {
                let msg = match event {
                    Event::Key(k) => Msg::Key(k),
                    Event::Mouse(m) => Msg::Mouse(m),
                    Event::Resize(w, h) => Msg::Resize(w, h),
                    _ => continue,
                };
                if tx_input.send(msg).is_err() { break; }
            }
        }
    });

    // Spawn tick thread (50ms)
    let tx_tick = tx.clone();
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(50));
            if tx_tick.send(Msg::Tick).is_err() { break; }
        }
    });

    // Spawn file watcher
    spawn_file_watcher(&app.config.vault_path, tx.clone())?;

    // Main loop — single-threaded, non-blocking
    loop {
        // Batch-drain all pending messages before rendering
        let mut msgs: SmallVec<[Msg; 16]> = SmallVec::new();
        msgs.push(rx.recv()?);            // block for at least one event
        while let Ok(msg) = rx.try_recv() { // drain non-blocking
            msgs.push(msg);
        }

        for msg in msgs {
            app.update(msg)?;
        }

        if app.should_quit { break; }

        terminal.draw(|f| app.view(f))?;
    }

    Ok(())
}
```

**Why this works without `tokio`:**

The event loop blocks on `mpsc::recv()` which yields the thread to the OS scheduler — zero CPU when idle. Background I/O (file saves, search) runs on `std::thread` spawns. The `Msg` channel acts as the single synchronization point. We avoid the 2-4ms `tokio` runtime boot penalty, keep the binary smaller (~3MB vs ~8MB with tokio), and eliminate async coloring.

The _only_ future scenario that warrants `tokio` is the NestJS sync connector (HTTP client). At that point, we add `tokio` behind a `sync-net` feature flag — it never touches the core event loop.

### 1.4 Module Layout

```
blackbox/
├── Cargo.toml
├── src/
│   ├── main.rs              # entry point, terminal setup, event loop
│   ├── app.rs               # App struct + ALL update/view/highlight logic (1,900+ lines)
│   │                        # NOTE: planned split into update/, view/, highlight/, utils/
│   │                        # modules (see Phase 3 roadmap below) is not yet done.
│   ├── model/
│   │   ├── mod.rs
│   │   ├── buffer.rs         # Buffer, Rope wrapper, Viewport, auto-save debounce
│   │   ├── cursor.rs         # CursorState, selection logic, Position
│   │   ├── mode.rs           # Mode enum (Normal/Insert/Sidebar/FinderOpen/Command/…)
│   │   └── config.rs         # AppConfig, deep-merge deserialization
│   ├── msg.rs                # Msg enum, Direction, PluginAction
│   └── plugin/
│       ├── mod.rs
│       ├── manager.rs        # PluginManager — discovery, command dispatch
│       ├── runtime.rs        # PluginRuntime — manifest read, lazy load, status
│       ├── manifest.rs       # PluginManifest, CommandDef, KeybindingDef, PluginId
│       ├── permission.rs     # Permission enum
│       ├── host_fns.rs       # HostFunctions stub (Phase 3)
│       └── installer.rs      # PluginInstaller stub (Phase 3)
├── config/
│   └── default.toml          # shipped default config
└── docs/
    ├── ARCHITECTURE.md       # this file
    └── CROSSCHECK.md         # README ↔ architecture alignment verification
```

> **Phase 3 target layout** (app.rs split — see §9 roadmap):
> `src/update/`, `src/view/`, `src/highlight/`, `src/utils/`

---

## 2. Crate Selection & Dependencies

### 2.1 Core

| Crate       | Version | Purpose            | Justification                                                                                       |
| ----------- | ------- | ------------------ | --------------------------------------------------------------------------------------------------- |
| `ratatui`   | `0.29+` | TUI framework      | Industry standard. Immediate-mode API fits MVU.                                                     |
| `crossterm` | `0.28+` | Terminal backend   | Cross-platform, no ncurses dependency.                                                              |
| `ropey`     | `1.6+`  | Text buffer (rope) | O(log n) edits, line indexing, `Chunks` for zero-copy iteration. Replaces `tui-textarea` internals. |

**Note on `tui-textarea`:** We use `ropey` directly instead. `tui-textarea` wraps a `Vec<String>` internally — this is `O(n)` on insert for large documents. We implement our own editor widget backed by `Rope`, borrowing cursor/selection logic from `tui-textarea`'s API surface but with a performant core.

### 2.2 Markdown & Syntax

| Crate            | Version | Purpose                                                  |
| ---------------- | ------- | -------------------------------------------------------- |
| `pulldown-cmark` | `0.12+` | Markdown AST parsing. Pull-parser, zero-alloc on borrow. |
| `syntect`        | `5.2+`  | Syntax highlighting for fenced code blocks.              |

### 2.3 Search

| Crate           | Version | Purpose                                                                                                                                                  |
| --------------- | ------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `fuzzy-matcher` | `0.3+`  | Filename fuzzy matching (Smith-Waterman / Skim algorithm).                                                                                               |
| `ignore`        | `0.4+`  | `.gitignore`-aware directory walking. Same engine as ripgrep.                                                                                            |
| `grep-regex`    | `0.1+`  | Content search with regex. ripgrep's core engine.                                                                                                        |
| `nucleo`        | `0.5+`  | **Alternative/upgrade path** — async fuzzy matcher used by Helix editor. Consider for Phase 2 if `fuzzy-matcher` FPS drops on large vaults (>10k files). |

### 2.4 Configuration & Serialization

| Crate         | Version | Purpose                                                                              |
| ------------- | ------- | ------------------------------------------------------------------------------------ |
| `serde`       | `1.0`   | Derive-based (de)serialization.                                                      |
| `toml`        | `0.8+`  | Config file format. Human-readable, Rust-native feel.                                |
| `directories` | `5.0+`  | XDG-compliant config/data paths (`~/.config/blackbox/`, `~/.local/share/blackbox/`). |

### 2.5 File System

| Crate    | Version | Purpose                                                                                    |
| -------- | ------- | ------------------------------------------------------------------------------------------ |
| `notify` | `7.0+`  | Cross-platform FS event watching. Debounced mode. Detects external edits (e.g., git pull). |

### 2.6 Utilities

| Crate              | Version | Purpose                                        |
| ------------------ | ------- | ---------------------------------------------- |
| `slotmap`          | `1.0`   | Generational arena for buffer IDs.             |
| `smallvec`         | `1.13+` | Stack-allocated vecs for message batching.     |
| `tracing`          | `0.1`   | Structured logging (file-based, never stdout). |
| `tracing-appender` | `0.2`   | Rolling file log output.                       |
| `anyhow`           | `1.0`   | Error handling in application code.            |
| `thiserror`        | `2.0+`  | Error types in library-style modules.          |

### 2.7 Plugin Runtime (Phase 3)

| Crate    | Version | Purpose                                                                                                                   |
| -------- | ------- | ------------------------------------------------------------------------------------------------------------------------- |
| `extism` | `1.13+` | WASM plugin runtime (chosen over raw `wasmtime`). Provides a high-level host-plugin RPC surface with built-in sandboxing. |

> **Note on wasmtime vs extism:** The original design specified `wasmtime` + `wit-bindgen` directly. `extism` is used instead — it wraps `wasmtime`'s Cranelift JIT internally and adds a simpler plugin API boundary without requiring WIT codegen. The plugin PDK contract is specified in `plugin.toml` manifests rather than WIT files.

### 2.8 Async Runtime Decision

**Decision: No `tokio` in core. `std::thread` + `mpsc` channels.**

Rationale:

1. TUI event loops are inherently single-consumer. `mpsc::Receiver::recv()` is the natural "select" primitive.
2. All I/O is either blocking-fast (local file read: <1ms) or fire-and-forget (file save on background thread).
3. `tokio` adds ~4.5MB to binary, ~2-4ms to startup, and introduces `Send + 'static` constraints that complicate the `Rope` borrow model.
4. `notify` v7 works with `std` callbacks — no async needed.
5. **Escape hatch:** When the NestJS sync connector lands, add `tokio` behind `#[cfg(feature = "sync-net")]` and spawn a dedicated sync thread running a tokio `LocalSet`. The core loop stays synchronous.

---

## 3. The "Pseudo-Rendering" Strategy

### 3.1 Pipeline

```
 Rope (source text)
   │
   ▼
 pulldown-cmark::Parser  ← iterates over rope.slice(..).chunks()
   │
   ▼
 Event stream: [Start(Heading(1)), Text("Title"), End(Heading(1)), ...]
   │
   ▼
 Style Mapper  ← converts AST events to ratatui::Style + span ranges
   │
   ▼
 HighlightCache  ← Vec<Vec<(Style, Range<usize>)>> per visible line
   │
   ▼
 view::editor  ← renders Line widgets from cache
```

### 3.2 AST → Style Mapping

```rust
use ratatui::style::{Color, Modifier, Style};

fn style_for_event(event: &Event, theme: &Theme) -> Style {
    match event {
        Event::Start(Tag::Heading { level: HeadingLevel::H1, .. }) =>
            Style::default()
                .fg(theme.h1_color)           // e.g., Color::Magenta
                .add_modifier(Modifier::BOLD),

        Event::Start(Tag::Heading { level: HeadingLevel::H2, .. }) =>
            Style::default()
                .fg(theme.h2_color)           // e.g., Color::Cyan
                .add_modifier(Modifier::BOLD),

        Event::Start(Tag::Emphasis) =>
            Style::default()
                .add_modifier(Modifier::ITALIC),

        Event::Start(Tag::Strong) =>
            Style::default()
                .add_modifier(Modifier::BOLD),

        Event::Start(Tag::Link { .. }) =>
            Style::default()
                .fg(theme.link_color)
                .add_modifier(Modifier::UNDERLINED),

        Event::Start(Tag::CodeBlock(kind)) =>
            Style::default()
                .fg(theme.code_fg)
                .bg(theme.code_bg),           // subtle background tint

        Event::Code => // inline code
            Style::default()
                .fg(theme.inline_code_fg)
                .bg(theme.inline_code_bg),

        Event::Start(Tag::BlockQuote(_)) =>
            Style::default()
                .fg(theme.blockquote_color)
                .add_modifier(Modifier::ITALIC),

        Event::Start(Tag::Item) =>
            Style::default()
                .fg(theme.list_bullet_color),

        // WikiLinks are parsed as raw text matching `\[\[.*?\]\]`
        // and styled separately in a post-processing pass.

        _ => Style::default(),
    }
}
```

### 3.3 Incremental Highlighting

Full document re-parse on every keystroke is unacceptable for large files. Strategy:

1. **Dirty line tracking:** On each edit, mark the edited line and all subsequent lines until the next block-level boundary as dirty in `HighlightCache.valid_range`.
2. **Viewport-only parsing:** Only parse and highlight lines `viewport.top_line..viewport.top_line + viewport.height + scroll_off`. Lines outside this range keep stale cache (invisible, irrelevant).
3. **Block boundary detection:** Fenced code blocks (` ``` `) and block quotes (`>`) are the only constructs that span multiple lines and can change highlight state. Track their start/end lines in a separate `block_index: Vec<BlockSpan>`. An edit inside a code block only invalidates that block.

```rust
impl HighlightCache {
    /// Re-highlight only the dirty visible lines.
    pub fn refresh(
        &mut self,
        rope: &Rope,
        viewport: &Viewport,
        theme: &Theme,
    ) {
        let vis_start = viewport.top_line;
        let vis_end = (vis_start + viewport.height as usize).min(rope.len_lines());

        // Skip if the visible range is fully valid
        if self.valid_range.start <= vis_start && self.valid_range.end >= vis_end {
            return;
        }

        // Parse only the visible slice
        let start_byte = rope.line_to_byte(vis_start);
        let end_byte = rope.line_to_byte(vis_end);
        let slice = rope.byte_slice(start_byte..end_byte);

        // pulldown-cmark can parse a &str — we collect chunks into a
        // small contiguous buffer only for the visible region.
        let visible_text: String = slice.chunks().collect();
        let parser = Parser::new_ext(&visible_text, Options::all());

        // ... map events to spans, populate self.spans[vis_start..vis_end]

        self.valid_range = vis_start..vis_end;
    }
}
```

### 3.4 Fenced Code Block Syntax Highlighting

For code blocks with a language tag (` ```rust `), delegate to `syntect`:

```rust
fn highlight_code_block(code: &str, lang: &str, theme: &SyntectTheme) -> Vec<Vec<(Style, String)>> {
    let ss = SyntaxSet::load_defaults_newlines();
    let syntax = ss.find_syntax_by_token(lang).unwrap_or_else(|| ss.find_syntax_plain_text());
    let mut h = HighlightLines::new(syntax, theme);

    code.lines()
        .map(|line| {
            h.highlight_line(line, &ss)
                .unwrap_or_default()
                .into_iter()
                .map(|(style, text)| (syntect_to_ratatui(style), text.to_string()))
                .collect()
        })
        .collect()
}
```

### 3.5 WikiLink Styling (Post-Pass)

WikiLinks (`[[Page Name]]`) are not standard Markdown. After the `pulldown-cmark` pass, run a regex scan on each visible line:

```rust
static WIKI_LINK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\]]+)\]\]").unwrap());

fn apply_wikilink_styles(line: &str, spans: &mut Vec<(Style, Range<usize>)>, theme: &Theme) {
    for m in WIKI_LINK_RE.find_iter(line) {
        spans.push((
            Style::default()
                .fg(theme.wikilink_color)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            m.range(),
        ));
    }
}
```

---

## 4. Plugin Architecture (WASM Sandbox)

### 4.1 Design Goals

- Plugins **cannot crash** the host. WASM sandboxing guarantees memory isolation.
- Plugins communicate via a **typed RPC boundary** defined in WIT (WASM Interface Types).
- Plugins can: read the active buffer (immutable view), propose edits (returned as diffs), draw to a dedicated pane, register commands.
- Plugins **cannot**: access the file system directly, make network calls, or mutate host state.

### 4.2 WIT Interface

```wit
// blackbox-plugin.wit
package blackbox:plugin@0.1.0;

interface host {
    /// Get the full text of the active buffer.
    get-buffer-text: func() -> string;

    /// Get the current cursor position.
    get-cursor: func() -> tuple<u32, u32>;

    /// Propose a text replacement. Host applies it via undo-able edit.
    propose-edit: func(start-line: u32, start-col: u32, end-line: u32, end-col: u32, new-text: string);

    /// Write styled text to the plugin's output pane.
    draw-pane: func(lines: list<styled-line>);

    /// Register a command in the command palette.
    register-command: func(name: string, description: string);

    /// Log a message to BlackBox's log file.
    log: func(level: log-level, message: string);
}

record styled-line {
    segments: list<styled-segment>,
}

record styled-segment {
    text: string,
    fg: option<color>,
    bg: option<color>,
    bold: bool,
    italic: bool,
}

enum color { red, green, blue, cyan, magenta, yellow, white, gray }
enum log-level { debug, info, warn, error }

world plugin {
    import host;

    /// Called once on plugin load.
    export init: func();

    /// Called when the user invokes a registered command.
    export on-command: func(name: string);

    /// Called on every buffer change (debounced, max 10Hz).
    export on-buffer-change: func();
}
```

### 4.3 Host-Side Runtime

```rust
pub struct PluginManager {
    engine: wasmtime::Engine,
    plugins: HashMap<PluginId, LoadedPlugin>,
}

struct LoadedPlugin {
    id: PluginId,
    manifest: PluginManifest,
    store: wasmtime::Store<PluginState>,
    instance: wasmtime::Instance,
}

#[derive(Deserialize)]
struct PluginManifest {
    name: String,
    version: String,
    wasm: PathBuf,          // relative to plugin dir
    permissions: Vec<Permission>,
}

#[derive(Deserialize)]
enum Permission {
    ReadBuffer,
    ProposeEdit,
    DrawPane,
    RegisterCommand,
}
```

**Execution model:** Plugin calls are synchronous and time-bounded. The host invokes `on_buffer_change()` on a background thread with a 50ms timeout via `wasmtime::Store::epoch_deadline_async`. If the plugin exceeds the deadline, the call is aborted and a warning is logged. The resulting `propose-edit` calls are queued as `Msg::PluginEvent` and applied on the main thread.

### 4.4 Plugin Discovery

```
~/.config/blackbox/plugins/
├── word-count/
│   ├── plugin.toml
│   └── word_count.wasm
└── markdown-fmt/
    ├── plugin.toml
    └── markdown_fmt.wasm
```

Plugins are loaded lazily on first use, not at startup (respects the <100ms boot constraint).

---

## 5. Configuration Schema

```toml
# ~/.config/blackbox/config.toml

[general]
vault_path = "~/notes"            # root directory for all notes
scratch_file = ".scratch.md"       # relative to vault_path
auto_save_debounce_ms = 300
theme = "cyberpunk"

[editor]
tab_width = 4
soft_wrap = true
line_numbers = false               # minimalist: off by default
scroll_off = 5

[keybinds]
# Vim-inspired defaults, fully remappable
normal.q = "quit"
normal.i = "mode:insert"
normal.slash = "finder:open"
normal.colon = "command:open"
insert.escape = "mode:normal"

[search]
max_results = 50
ignore_patterns = [".git", "node_modules", ".obsidian"]

[sync]
backend = "git"                    # "git" | "none" | "nestjs" (future)

[sync.git]
auto_commit = true
auto_push = false
commit_message_format = "blackbox: auto-save {timestamp}"

[theme.cyberpunk]
bg = "#0a0a0f"
fg = "#e0e0e0"
h1 = "#ff00ff"
h2 = "#00ffff"
link = "#ff6600"
wikilink = "#00ff88"
code_bg = "#1a1a2e"
code_fg = "#c0c0c0"
cursor = "#ff0044"
selection = "#2a2a4e"
status_bg = "#1a1a2e"
status_fg = "#888888"
```

```rust
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub editor: EditorConfig,
    pub keybinds: KeybindConfig,
    pub search: SearchConfig,
    pub sync: SyncConfig,
    pub theme: HashMap<String, ThemeConfig>,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let xdg = directories::ProjectDirs::from("", "", "blackbox")
            .ok_or_else(|| anyhow!("cannot determine config directory"))?;

        let config_path = xdg.config_dir().join("config.toml");

        // Layer: defaults → user config → env overrides
        let defaults = include_str!("../config/default.toml");
        let mut config: AppConfig = toml::from_str(defaults)?;

        if config_path.exists() {
            let user = fs::read_to_string(&config_path)?;
            let user_config: AppConfig = toml::from_str(&user)?;
            config.merge(user_config);
        }

        Ok(config)
    }
}
```

---

## 6. The "Never-Lost" Buffer

### 6.1 Auto-Save Strategy

```rust
impl App {
    fn handle_buffer_edit(&mut self, buffer_id: BufferId) {
        let buf = &mut self.buffers[buffer_id];
        buf.dirty = true;
        buf.highlights.invalidate(buf.cursor.row);

        // Reset debounce timer
        buf.save_debounce = Some(Instant::now() + Duration::from_millis(
            self.config.general.auto_save_debounce_ms
        ));
    }

    fn handle_tick(&mut self) {
        let now = Instant::now();
        let saves: SmallVec<[BufferId; 4]> = self.buffers.iter()
            .filter(|(_, b)| b.save_debounce.map_or(false, |t| now >= t))
            .map(|(id, _)| id)
            .collect();

        for id in saves {
            self.schedule_save(id);
        }
    }

    fn schedule_save(&mut self, id: BufferId) {
        let buf = &mut self.buffers[id];
        buf.save_debounce = None;
        buf.dirty = false;

        let path = buf.path.clone().unwrap_or_else(|| {
            self.config.general.vault_path.join(&self.config.general.scratch_file)
        });

        // Snapshot the rope for background write
        let rope_clone = buf.rope.clone(); // Rope::clone is O(1) — COW internals

        let tx = self.event_tx.clone();
        thread::spawn(move || {
            let result = (|| -> Result<()> {
                let tmp = path.with_extension("tmp");
                let file = File::create(&tmp)?;
                let mut writer = BufWriter::new(file);
                for chunk in rope_clone.chunks() {
                    writer.write_all(chunk.as_bytes())?;
                }
                writer.flush()?;
                fs::rename(&tmp, &path)?; // atomic replace
                Ok(())
            })();

            if let Err(e) = result {
                let _ = tx.send(Msg::Notify(format!("Save failed: {e}")));
            }
        });
    }
}
```

**Key detail:** `Rope::clone()` is **O(1)** due to copy-on-write reference counting on internal tree nodes. We clone the rope, hand it to a background thread, and write it out. The main thread is never blocked by I/O. The atomic `rename` ensures we never leave a half-written file.

### 6.2 Scratch Bootstrap

On startup, if `~/.blackbox/.scratch.md` exists, load it. If not, create it with a timestamp header. The scratch buffer is **always** at `buffers.scratch` and cannot be closed — only cleared.

---

## 7. WikiLink Navigation

```rust
fn follow_wikilink(app: &mut App, link_text: &str) -> Result<()> {
    let vault = &app.config.general.vault_path;

    // Resolution order:
    // 1. Exact match: `{link_text}.md`
    // 2. Case-insensitive match
    // 3. Subdirectory search: `**/{link_text}.md`
    let candidates = [
        vault.join(format!("{link_text}.md")),
    ];

    for path in &candidates {
        if path.exists() {
            app.open_file(path)?;
            return Ok(());
        }
    }

    // Fallback: walk with `ignore` crate
    let target = format!("{}.md", link_text.to_lowercase());
    for entry in WalkBuilder::new(vault).build().flatten() {
        if entry.file_name().to_str()
            .map_or(false, |n| n.to_lowercase() == target)
        {
            app.open_file(entry.path())?;
            return Ok(());
        }
    }

    // Not found → create new note
    let new_path = vault.join(format!("{link_text}.md"));
    fs::write(&new_path, format!("# {link_text}\n\n"))?;
    app.open_file(&new_path)?;

    Ok(())
}
```

---

## 8. Sync Layer

### 8.1 Git Sync

```rust
pub struct GitSync {
    repo_path: PathBuf,
    config: GitSyncConfig,
}

impl GitSync {
    /// Called after successful file save (debounced separately at ~30s).
    pub fn auto_commit(&self) -> Result<()> {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let msg = self.config.commit_message_format
            .replace("{timestamp}", &timestamp.to_string());

        // Shell out to git — avoids libgit2 dependency (~2MB).
        // These are fast local operations.
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&self.repo_path)
            .output()?;

        Command::new("git")
            .args(["commit", "-m", &msg, "--allow-empty-message"])
            .current_dir(&self.repo_path)
            .output()?;

        if self.config.auto_push {
            Command::new("git")
                .args(["push"])
                .current_dir(&self.repo_path)
                .output()?;
        }

        Ok(())
    }
}
```

### 8.2 Sync Trait (Future Extensibility)

```rust
pub trait SyncBackend: Send + Sync {
    fn sync(&self) -> Result<SyncResult>;
    fn status(&self) -> SyncStatus;
}

pub enum SyncResult {
    NoChanges,
    Committed { hash: String },
    Pushed,
    Conflict(Vec<PathBuf>),
}

pub enum SyncStatus {
    Idle,
    Syncing,
    Error(String),
}
```

The `NestJsSync` impl will land in Phase 3+ behind the `sync-net` feature flag with `tokio` + `reqwest`.

---

## 9. Phased Implementation Roadmap

### Phase 1: The MVP — "Scratch That"

**Duration:** 4-6 weeks
**Goal:** A usable single-buffer markdown scratchpad with auto-save.

| Week | Task                                                                                                                      | Deliverable                                    |
| ---- | ------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------- |
| 1    | Project scaffolding. Cargo workspace, CI (GitHub Actions), `cargo clippy` / `cargo fmt` gates.                            | Compiling skeleton with `ratatui` hello-world. |
| 2    | Core MVU loop. `App`, `Msg`, `update()`, `view()`. Render a rope-backed buffer with cursor movement (hjkl/arrows).        | Editable text buffer in terminal.              |
| 3    | Markdown pseudo-rendering. `pulldown-cmark` integration, viewport-only highlighting. Headings, bold, italic, code, lists. | Styled markdown in the editor pane.            |
| 4    | Auto-save + scratch buffer. Debounced save, atomic write, scratch bootstrap, `notify` watcher for external changes.       | "Never-Lost" buffer fully functional.          |
| 5    | Config system. TOML loading, theme support, keybind remapping, XDG paths.                                                 | Configurable app.                              |
| 6    | Polish. Status bar (mode, file, save indicator). Mouse scroll support. Error handling + `tracing` logging.                | **MVP Release: `v0.1.0`**                      |

**Exit criteria:** `cargo install blackbox-tui`, open it, type notes, close terminal, reopen — notes are there. <100ms startup measured with `hyperfine`.

### Phase 2: The Structure — "Find Anything"

**Duration:** 4-6 weeks
**Goal:** Multi-buffer editing, file navigation, wiki-links.

| Week | Task                                                                                                   | Deliverable                        |
| ---- | ------------------------------------------------------------------------------------------------------ | ---------------------------------- |
| 1    | Multi-buffer `SlotMap`. Buffer switching (`Ctrl+N` / `Ctrl+P`), tab bar or buffer list in status.      | Multiple open files.               |
| 2    | File tree sidebar. `ignore` crate directory walker, collapsible tree widget, open on Enter.            | Sidebar navigation.                |
| 3    | Fuzzy finder overlay. `fuzzy-matcher` on filenames, ranked results, preview pane.                      | `Ctrl+/` fuzzy file picker.        |
| 4    | WikiLink support. `[[Link]]` detection, follow-link (`gd` or Enter in Normal mode), create-if-missing. | Linked knowledge graph navigation. |
| 5    | Content search. `grep-regex` full-text search, results in finder overlay.                              | `Ctrl+Shift+F` content search.     |
| 6    | `syntect` code blocks. Language-aware highlighting inside fenced blocks.                               | **Structure Release: `v0.2.0`**    |

**Exit criteria:** Open BlackBox, `Ctrl+/` to find a note, follow a `[[WikiLink]]` to another note, search for a phrase across all notes.

### Phase 3: The Ecosystem — "Extend Everything"

**Duration:** 6-8 weeks
**Goal:** Plugin system, advanced features, sync.

| Week | Task                                                                                    | Deliverable                           |
| ---- | --------------------------------------------------------------------------------------- | ------------------------------------- |
| 1    | Split `app.rs` into `update/`, `view/`, `highlight/`, `utils/` modules (pure refactor). | Modular codebase ready for extension. |
| 2    | Undo/redo with `UndoTree` in `Buffer`. Persist undo history across saves.               | Full undo/redo support.               |
| 3    | WASM plugin runtime. `extism` integration, plugin manifest loading, command dispatch.   | Host can load and call a WASM plugin. |
| 4    | Plugin API: `get-buffer-text`, `propose-edit`, `draw-pane`. Sandbox time limits.        | Plugins can read/modify buffers.      |
| 5    | Example plugins: word-count, markdown-fmt.                                              | Proof-of-concept ecosystem.           |
| 6    | Git sync integration. Auto-commit, auto-push, conflict detection, status in statusbar.  | Git-based sync working.               |
| 7    | Wire unused config fields: `tab_width`, `line_numbers`, `ignore_patterns`, `soft_wrap`. | Config actually does what it says.    |
| 8    | Polish, docs, website, `crates.io` publish.                                             | **Ecosystem Release: `v0.3.0`**       |

**Exit criteria:** Third-party developer can write a plugin in Rust, compile to WASM, drop it in `~/.config/blackbox/plugins/`, and it runs.

---

## 10. Performance Targets

| Metric                              | Target              | How to Measure                                   |
| ----------------------------------- | ------------------- | ------------------------------------------------ |
| Cold startup → first frame          | <100ms              | `hyperfine 'blackbox'`                           |
| Keystroke → re-render latency       | <16ms (60fps)       | Internal `tracing` spans                         |
| File save (10KB note)               | <5ms on main thread | Async — main thread cost is `Rope::clone()` only |
| Fuzzy search (10K files)            | <50ms               | Benchmark with `criterion`                       |
| Binary size (release, stripped)     | <10MB               | `cargo bloat`, `strip -s`                        |
| Memory (100 open buffers, avg 10KB) | <50MB RSS           | `heaptrack` / Activity Monitor                   |

---

## 11. CI / Quality Gates

```yaml
# .github/workflows/ci.yml
on: [push, pull_request]
jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings
      - run: cargo test
      - run: cargo build --release
      - run: hyperfine --warmup 3 './target/release/blackbox --version'
```

---

## Appendix A: Key Shortcuts (Default)

| Mode   | Key        | Action                       |
| ------ | ---------- | ---------------------------- |
| Normal | `i`        | Enter Insert mode            |
| Normal | `q`        | Quit (confirms if dirty)     |
| Normal | `/`        | Open fuzzy finder            |
| Normal | `:`        | Open command palette         |
| Normal | `gd`       | Follow WikiLink under cursor |
| Normal | `Ctrl+S`   | Force save                   |
| Normal | `Ctrl+N`   | Next buffer                  |
| Normal | `Ctrl+P`   | Previous buffer              |
| Normal | `Ctrl+E`   | Toggle sidebar               |
| Insert | `Esc`      | Return to Normal mode        |
| Finder | `Esc`      | Close finder                 |
| Finder | `Enter`    | Open selected file           |
| Finder | `Ctrl+J/K` | Navigate results             |

---

_This document is a living specification. Update it as implementation reveals new constraints._
