# ■ BlackBox

> **A cyberpunk TUI knowledge base. Local first, sync second.**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.83%2B-orange.svg)](https://www.rust-lang.org/)

```
  ██████  ██       █████   ██████ ██   ██ ██████   ██████  ██   ██
  ██   ██ ██      ██   ██ ██      ██  ██  ██   ██ ██    ██  ██ ██
  ██████  ██      ███████ ██      █████   ██████  ██    ██   ███
  ██   ██ ██      ██   ██ ██      ██  ██  ██   ██ ██    ██  ██ ██
  ██████  ███████ ██   ██  ██████ ██   ██ ██████   ██████  ██   ██
```

A **fast**, **local-first** markdown knowledge base that lives in your terminal. Built for the paranoid note-taker who refuses to trust the cloud with their thoughts.

## ⚡ Why BlackBox?

- **🔒 Local First** — Your notes are **yours**. Plain markdown files in `~/.blackbox/`. No database, no vendor lock-in, no sync servers reading your journal entries.
- **🚀 Fast** — Sub-100ms startup. No Electron. No JavaScript. Just Rust and your terminal.
- **💾 Never Lost** — Aggressive auto-save with debouncing. External file watching. Quit confirmation for dirty buffers. Your thoughts don't evaporate.
- **🎨 Markdown Native** — Pseudo-rendering in the terminal: headings, bold, italic, links, code blocks, and `[[WikiLinks]]` styled in real-time.
- **🔍 Fuzzy Everything** — `/` to fuzzy-find files. `Ctrl+Shift+F` for full-text content search across your entire vault.
- **📑 Multi-Buffer Tabs** — Work on multiple notes simultaneously. Switch with `Ctrl+N`/`Ctrl+P`. All tabs auto-save independently.
- **⌨️ Vim-Style** — Modal editing (Normal/Insert/Sidebar). `hjkl` navigation. `i` to insert, `Esc` to escape. You know the drill.

## 📦 Installation

### From Source

```bash
git clone https://github.com/jcyrus/blackbox.git
cd blackbox
cargo build --release
./target/release/blackbox
```

### From Cargo (Coming Soon)

```bash
cargo install blackbox-tui
```

## 🎮 Quick Start

1. **Launch BlackBox:**

   ```bash
   blackbox
   ```

2. **Start typing** in the scratch buffer (it's already open). Notes auto-save.

3. **Key Shortcuts:**

| Mode      | Key            | Action                             |
| --------- | -------------- | ---------------------------------- |
| Normal    | `i`            | Enter Insert mode                  |
| Normal    | `Esc`          | Return to Normal mode              |
| Normal    | `q`            | Quit (confirms if unsaved)         |
| Normal    | `Q`            | Save all & quit immediately        |
| Normal    | `Ctrl+E`       | Toggle sidebar                     |
| Normal    | `/`            | Fuzzy file finder                  |
| Normal    | `Ctrl+Shift+F` | Full-text content search           |
| Normal    | `Ctrl+N/P`     | Next/previous buffer tab           |
| Normal    | `Ctrl+S`       | Force save current buffer          |
| Normal    | `gd`           | Follow `[[WikiLink]]` under cursor |
| Normal    | `Ctrl+B`       | Toggle backlinks panel             |
| Normal    | `hjkl`         | Cursor navigation (or arrows)      |
| Normal    | `0` / `$`      | Jump to line start / end           |
| Sidebar   | `n`            | Create new file                    |
| Sidebar   | `N`            | Create new folder                  |
| Sidebar   | `Enter`        | Open selected file                 |
| Backlinks | `j/k`          | Navigate linking notes             |
| Backlinks | `Enter`        | Jump to linking note               |
| Insert    | `Esc`          | Return to Normal mode              |

4. **Fuzzy Search:**
   - `/` opens the file finder
   - `Ctrl+Shift+F` opens content search (grep across all files)
   - `j/k` or arrow keys to navigate results
   - `Enter` to open selected file
   - `Esc` to close finder

## 🏗️ Architecture

BlackBox is built on a strict **Elm Architecture (MVU)** pattern:

- **Model:** Single source of truth (`App` struct with rope-backed buffers)
- **Message:** All state changes flow through a typed `Msg` enum
- **View:** Pure rendering function — no side effects

**Tech Stack:**

- 🦀 Rust 1.83+ (edition 2024)
- 📟 `ratatui` for immediate-mode TUI rendering
- 🪢 `ropey` for rope-based text buffers (O(log n) edits)
- 🔍 `fuzzy-matcher` for file search (Skim algorithm)
- 👀 `notify` for file system watching
- 📝 `pulldown-cmark` for markdown parsing

**Design Philosophy:**

- **No async runtime** — `std::thread` + `mpsc::channel` keeps startup <100ms
- **No database** — The file system _is_ the database
- **Zero-copy** — Markdown rendering uses `&str` borrows from the rope buffer
- **Incremental rendering** — Only re-parse visible viewport on edits

📖 **Deep dive:** See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full technical spec.

## ✨ Feature Status

### ✅ Phase 1: The MVP — "Scratch That" (COMPLETE)

- [x] Rope-backed text buffer with cursor movement
- [x] Markdown pseudo-rendering (headings, bold, italic, code blocks)
- [x] Auto-save with debouncing
- [x] Scratch buffer ("Never-Lost" inbox)
- [x] File watching for external changes
- [x] Config system (TOML)
- [x] Quit confirmation for dirty buffers

### 🚧 Phase 2: The Structure — "Find Anything" (IN PROGRESS)

- [x] Multi-buffer tabs with `SlotMap`
- [x] File tree sidebar
- [x] Fuzzy file finder
- [x] Content search (full-text grep)
- [x] Inline file/folder creation from sidebar
- [x] `[[WikiLink]]` navigation — `gd` to follow, `y/n` prompt to create missing notes
- [x] `syntect` syntax highlighting for fenced code blocks (language-aware)
- [x] Backlinks panel — `Ctrl+B` to toggle, shows all notes linking to current

### 🔮 Phase 3: The Ecosystem — "Extend Everything" (PLANNED)

- [ ] WASM plugin system
- [ ] Git-based sync
- [ ] Undo tree visualization
- [ ] Plugin API for buffer manipulation
- [ ] Conflict resolution UI

## 🎨 Customization

Edit `~/.config/blackbox/config.toml`:

```toml
vault_path = "~/.blackbox"  # Where your notes live

[theme]
heading_color = "Magenta"
link_color = "Cyan"
code_color = "Yellow"

[editor]
tab_width = 4
line_numbers = false
scroll_context = 3  # Lines of context above/below cursor
```

## 🤝 Contributing

BlackBox is in active development! Contributions welcome:

1. Check [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the technical roadmap
2. Look for issues tagged `good-first-issue`
3. Follow the MVU pattern strictly (all mutations through `Msg`)
4. Run `cargo fmt` and `cargo clippy` before submitting

## 📜 License

MIT License — see [LICENSE](LICENSE) for details.

## 🧠 Philosophy

> _"Your thoughts are your own. Your notes should be too."_

BlackBox is for people who:

- Don't trust Notion with their personal journal
- Want Obsidian's local-first philosophy in a terminal
- Believe markdown files should outlive any app
- Think 1GB Electron apps are a crime against computing
- Value speed, simplicity, and owning their data

If you're reading this, you're probably one of us. Welcome to the box.

---

**Status:** Phase 1 complete. Phase 2 complete. Phase 3 next — watch this space.

_Built with ☕ and Rust by [@jcyrus](https://github.com/jcyrus)_
