# Changelog

All notable changes to BlackBox will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-02-17

### Added

#### Core Editor

- Rope-backed text buffer using `ropey` for O(log n) insert/delete operations
- Modal editing with Normal, Insert, and Sidebar modes
- Vim-style cursor navigation (`hjkl`, arrow keys, `0`, `$`)
- Multi-buffer tab system with independent state per buffer
- Tab switching with `Ctrl+N` (next) and `Ctrl+P` (previous)

#### Markdown Support

- Real-time pseudo-rendering of markdown in the terminal
- Styled headings (H1-H6) with configurable colors
- Bold (`**text**`), italic (`*text*`), and inline code rendering
- WikiLink detection (`[[Link]]`) with visual styling
- Regular markdown link rendering
- Fenced code block support with visual distinction

#### File Management

- Scratch buffer that auto-loads from `~/.blackbox/Scratch.md`
- File tree sidebar with recursive directory navigation
- Toggle sidebar visibility with `Ctrl+E`
- Open files with `Enter` in sidebar
- Create new files inline with `n` key in sidebar
- Create new folders inline with `N` key in sidebar
- File system watching with `notify` crate for external change detection
- Auto-reload buffers when files change externally

#### Auto-Save

- Aggressive auto-save with 2-second debouncing
- Independent auto-save for all open buffers (active and inactive)
- Dirty buffer tracking with visual indicators
- Atomic file writes to prevent corruption

#### Search

- Fuzzy file finder with `/` shortcut
- Full-text content search across vault with `Ctrl+Shift+F`
- Skim algorithm for fast fuzzy matching
- File path and content preview in search results
- Navigate results with `j`/`k` or arrow keys
- Jump to specific line when opening from content search

#### Configuration

- TOML-based configuration system
- XDG-compliant config paths (`~/.config/blackbox/`)
- Configurable vault path (default: `~/.blackbox/`)
- Configurable theme colors for markdown elements
- Configurable editor settings (tab width, scroll context, etc.)
- Ships with sensible defaults in `config/default.toml`

#### Safety Features

- Quit confirmation when unsaved changes exist
- Two-step quit: press `q` twice within 2 seconds to save and quit
- Fast quit with `Q` (Shift-Q) to save all and quit immediately
- Status bar shows pending write count during quit confirmation
- Manual save with `Ctrl+S`

#### Architecture

- Elm/MVU (Model-View-Update) architecture for predictable state management
- No async runtime - uses `std::thread` + `mpsc::channel` for sub-100ms startup
- Zero-copy markdown rendering using `&str` borrows from rope buffer
- Incremental render cache - only re-parse viewport on edits
- Event-driven design with typed message passing

### Technical Details

- Built with Rust 2024 edition
- Uses `ratatui` 0.29 for TUI rendering
- Uses `crossterm` 0.28 for cross-platform terminal support
- Uses `pulldown-cmark` 0.12 for markdown parsing
- Uses `fuzzy-matcher` 0.3 with Skim algorithm for search
- Binary size: ~3-4MB (release build, stripped)
- Target startup time: <100ms on modern hardware

### Known Limitations

- WikiLink navigation (`gd` to follow links) not yet implemented
- No syntax highlighting for code blocks yet (planned with `syntect`)
- No backlinks panel yet
- No undo/redo system yet
- No plugin system yet
- No git-based sync yet

[0.1.0]: https://github.com/jcyrus/blackbox/releases/tag/v0.1.0
