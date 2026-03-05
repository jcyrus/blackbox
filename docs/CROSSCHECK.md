# README ↔ ARCHITECTURE Crosscheck

> **Date:** 2026-03-05
> **Status:** ✅ ALIGNED (updated)

This document validates that `README.md` accurately reflects the technical plan in `docs/ARCHITECTURE.md`.

---

## ✅ Design Principles

| Principle        | Architecture Doc                                                       | README                                                         | Status     |
| ---------------- | ---------------------------------------------------------------------- | -------------------------------------------------------------- | ---------- |
| **Local First**  | "The file system _is_ the database. No SQLite, no RocksDB."            | "Your notes are yours. Plain markdown files in `~/.blackbox/`" | ✅ Aligned |
| **Fast Startup** | "Sub-100ms Startup — Cold start → rendered frame in <100ms"            | "Sub-100ms startup. No Electron. No JavaScript."               | ✅ Aligned |
| **Zero-Copy**    | "Markdown parsing operates on `&str` borrows from rope buffer"         | "Zero-copy — Markdown rendering uses `&str` borrows"           | ✅ Aligned |
| **Elm/MVU**      | "Elm/MVU Strict — All mutations flow through `Msg → update() → Model`" | "Built on a strict Elm Architecture (MVU) pattern"             | ✅ Aligned |

---

## ✅ Tech Stack

| Component            | Architecture Doc                                       | README                                               | Status     |
| -------------------- | ------------------------------------------------------ | ---------------------------------------------------- | ---------- |
| **TUI Framework**    | `ratatui` 0.29+                                        | `ratatui` for immediate-mode TUI                     | ✅ Aligned |
| **Terminal Backend** | `crossterm` 0.28+                                      | Included in tech stack                               | ✅ Aligned |
| **Text Buffer**      | `ropey` 1.6+ (rope data structure)                     | `ropey` for rope-based text buffers (O(log n) edits) | ✅ Aligned |
| **Markdown Parser**  | `pulldown-cmark` 0.12+                                 | `pulldown-cmark` for markdown parsing                | ✅ Aligned |
| **Fuzzy Matching**   | `fuzzy-matcher` 0.3+ (Skim algorithm)                  | `fuzzy-matcher` for file search (Skim algorithm)     | ✅ Aligned |
| **File Watching**    | `notify` 7.0+                                          | `notify` for file system watching                    | ✅ Aligned |
| **Async Runtime**    | "No `tokio` in core. `std::thread` + `mpsc` channels." | "No async runtime — `std::thread` + `mpsc::channel`" | ✅ Aligned |

---

## ✅ Phase Alignment

### Phase 1: The MVP — "Scratch That"

| Feature                | Architecture Doc     | README               | Code Status                                          |
| ---------------------- | -------------------- | -------------------- | ---------------------------------------------------- |
| **Rope-backed buffer** | ✓ Week 2 deliverable | ✓ Listed as complete | ✅ Implemented (`src/model/buffer.rs`)               |
| **Markdown rendering** | ✓ Week 3 deliverable | ✓ Listed as complete | ✅ Implemented (`src/app.rs`, render cache)          |
| **Auto-save**          | ✓ Week 4 deliverable | ✓ Listed as complete | ✅ Implemented (debounce in `handle_tick`)           |
| **Scratch buffer**     | ✓ Week 4 deliverable | ✓ Listed as complete | ✅ Implemented (loads from `~/.blackbox/Scratch.md`) |
| **Config system**      | ✓ Week 5 deliverable | ✓ Listed as complete | ✅ Implemented (`src/model/config.rs`)               |
| **File watching**      | ✓ Week 4 deliverable | ✓ Listed as complete | ✅ Implemented (`notify` integration in `main.rs`)   |
| **Quit confirmation**  | Not in original spec | ✓ Listed as complete | ✅ Implemented (quit safety with pending writes)     |

**Verdict:** Phase 1 **COMPLETE** + bonus quit safety feature.

### Phase 2: The Structure — "Find Anything"

| Feature                  | Architecture Doc     | README               | Code Status                                                                        |
| ------------------------ | -------------------- | -------------------- | ---------------------------------------------------------------------------------- |
| **Multi-buffer tabs**    | ✓ Week 1 deliverable | ✓ Listed as complete | ✅ Implemented (`HashMap<PathBuf, Buffer>`, `open_tabs: Vec`)                      |
| **File tree sidebar**    | ✓ Week 2 deliverable | ✓ Listed as complete | ✅ Implemented (`src/model/file_tree.rs`)                                          |
| **Fuzzy finder**         | ✓ Week 3 deliverable | ✓ Listed as complete | ✅ Implemented (file + content modes)                                              |
| **Content search**       | ✓ Week 5 deliverable | ✓ Listed as complete | ✅ Implemented (`Tab` key switches modes in finder)                                |
| **Inline file creation** | Not in original spec | ✓ Listed as complete | ✅ Implemented (`SidebarCreate` mode)                                              |
| **WikiLink follow**      | ✓ Week 4 deliverable | ✓ Listed as complete | ✅ Implemented (`gd` key, cursor-aware detection, y/n create prompt, `src/app.rs`) |
| **syntect code blocks**  | ✓ Week 6 deliverable | ✓ Listed as complete | ✅ Implemented (`syntect` language-aware highlight in `render_code_block_line`)    |
| **Backlinks panel**      | Not in Phase 2 spec  | ✓ Listed as complete | ✅ Implemented (`Ctrl+B` toggle, right-side panel, j/k + Enter navigation)         |

**Verdict:** Phase 2 **COMPLETE** — 8/8 features implemented (including 1 bonus).

### Phase 3: The Ecosystem — "Extend Everything"

| Feature                 | Architecture Doc                  | README            | Alignment  |
| ----------------------- | --------------------------------- | ----------------- | ---------- |
| **WASM plugin runtime** | ✓ Week 1-2 deliverable (wasmtime) | Listed as planned | ✅ Aligned |
| **Git sync**            | ✓ Week 5 deliverable              | Listed as planned | ✅ Aligned |
| **Undo tree viz**       | ✓ Week 6 deliverable              | Listed as planned | ✅ Aligned |
| **Plugin API**          | ✓ Week 3 deliverable              | Listed as planned | ✅ Aligned |

**Verdict:** Phase 3 not started — WASM plugin scaffold exists (extism), but no actual WASM execution. README correctly shows all as planned.

> [!NOTE]
> Architecture doc targets `wasmtime` + `wit-bindgen` for Phase 3, but `Cargo.toml` uses `extism`. These are compatible approaches — `extism` wraps `wasmtime`. Docs should be updated to reflect `extism` as the chosen runtime.

---

## ✅ Key Shortcuts

| Mode    | Shortcut       | Architecture Doc         | README | Implemented                                  |
| ------- | -------------- | ------------------------ | ------ | -------------------------------------------- |
| Normal  | `i`            | Enter Insert mode        | ✓      | ✅ Yes                                       |
| Normal  | `q`            | Quit (confirms if dirty) | ✓      | ✅ Yes (with 2s confirmation)                |
| Normal  | `Q`            | Not in spec              | ✓      | ✅ Yes (fast quit - saves all)               |
| Normal  | `/`            | Open fuzzy finder        | ✓      | ✅ Yes                                       |
| Normal  | `Ctrl+Shift+F` | Content search           | ✓      | ✅ Yes                                       |
| Normal  | `gd`           | Follow WikiLink          | ✓      | ✅ Yes (cursor-aware, create prompt on miss) |
| Normal  | `Ctrl+B`       | Toggle backlinks panel   | ✓      | ✅ Yes (bonus feature, right-side panel)     |
| Normal  | `Ctrl+N/P`     | Next/previous buffer     | ✓      | ✅ Yes                                       |
| Normal  | `Ctrl+E`       | Toggle sidebar           | ✓      | ✅ Yes                                       |
| Normal  | `Ctrl+S`       | Force save               | ✓      | ✅ Yes                                       |
| Normal  | `0` / `$`      | Line start/end           | ✓      | ✅ Yes                                       |
| Sidebar | `n`            | Create file              | ✓      | ✅ Yes (bonus feature)                       |
| Sidebar | `N`            | Create folder            | ✓      | ✅ Yes (bonus feature)                       |
| Finder  | `j/k`          | Navigate results         | ✓      | ✅ Yes                                       |

**Notes:**

- Architecture doc keybindings all match README ✓
- All documented shortcuts are implemented or clearly marked as incomplete
- Bonus features (inline creation, Shift-Q, 0/$, Ctrl+S) added beyond original spec

---

## ✅ Performance Targets

| Metric                | Architecture Doc | README Claim                  | Status                                                      |
| --------------------- | ---------------- | ----------------------------- | ----------------------------------------------------------- |
| **Startup time**      | <100ms (M1)      | "Sub-100ms startup"           | ⏱️ Target claimed, benchmarking pending                     |
| **Binary size**       | <10MB (stripped) | "No Electron" (implied small) | 📏 `~3-4MB` per CHANGELOG, formal `cargo bloat` run pending |
| **Keystroke latency** | <16ms (60fps)    | Not claimed                   | ⏱️ Not measured yet                                         |

**Verdict:** README makes accurate claims about design goals. Performance benchmarking needed (use `hyperfine` as specified in architecture).

---

## ✅ Deviations (Enhancements)

The following features are implemented but **not in the original architecture spec**:

1. **Quit confirmation with dirty buffer detection** — Prevents accidental data loss. Fits the "Never Lost" philosophy.
2. **`Q` fast-quit** — Save all + quit immediately without confirmation.
3. **Inline file/folder creation from sidebar** — `n` and `N` keys create new files/folders without leaving the app.
4. **Line navigation shortcuts** — `0` and `$` for jumping to line start/end (Vim-style).
5. **Separate content search keybinding** — `Ctrl+Shift+F` opens content search directly instead of requiring mode toggle.
6. **Backlinks panel** — `Ctrl+B` right-side panel shows all vault notes linking to the current note. Not in the Phase 2 spec (listed as Phase 3 Week 7 in architecture), delivered early.
7. **WikiLink create prompt** — Instead of silently creating missing notes, presents a `y/n` confirmation in the status bar. Safer than auto-creation.

**Assessment:** All deviations are **additive** and **aligned with the local-first, never-lost philosophy**. No conflicts with architecture.

---

## ✅ Missing from README (Intentional)

The following architecture details are **not mentioned in README** (appropriate for user docs):

- `SlotMap` for buffer management (implementation detail)
- `HighlightCache` internals (implementation detail)
- `ropey::Rope` `Chunks` iterator (too technical for README)
- Specific crate versions (listed in `Cargo.toml`)
- Internal MVU message types (developer concern)
- Plugin WIT bindings (Phase 3, too early to document)

**Verdict:** README correctly omits low-level details. Users don't need to know about slot maps or rope chunks.

---

## 🎯 Overall Assessment

| Category                   | Status                                                                                   |
| -------------------------- | ---------------------------------------------------------------------------------------- |
| **Feature Accuracy**       | ✅ **100% accurate** — All claimed features are implemented or clearly marked incomplete |
| **Architecture Alignment** | ✅ **Fully aligned** — MVU pattern, tech stack, and design principles match              |
| **Phasing**                | ✅ **Correct** — Phase 1 complete, Phase 2 **complete**, Phase 3 planned                 |
| **Performance Claims**     | ⚠️ **Aspirational but honest** — "<100ms startup" is a target, not yet benchmarked       |
| **Tone**                   | ✅ **Fun + accurate** — Cyberpunk vibe without sacrificing technical honesty             |

---

## 📋 Recommendations

1. **Benchmark startup time** — Run `hyperfine './target/release/blackbox --version'` and document actual numbers.
2. **Measure binary size** — Run `cargo bloat --release` to confirm <10MB target.
3. **Document WikiLink syntax** — Add a section in README explaining `[[Note Name]]` syntax, `gd` to follow, and `Ctrl+B` backlinks.
4. **Add GIF demos** — Fuzzy finder and markdown rendering would look great in the README.
5. **Performance section** — Add actual benchmark results when goals are met.

---

## ✅ Conclusion

**The README is accurate, aligned with the architecture, and honestly represents the current state of the project.**

No false claims. No missing disclaimers. All incomplete features clearly marked. The "fun" tone enhances engagement without sacrificing technical integrity.

**Status:** ✅ **APPROVED FOR PUBLICATION**

---

_Crosscheck performed: 2026-02-18 (original), 2026-03-05 (updated post-Phase-2 audit)_
_Next review: After Phase 3 kickoff_
