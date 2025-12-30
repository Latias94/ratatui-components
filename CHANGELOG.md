# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
once it is released.

## [0.1.0] - Unreleased

This project has not been released yet. APIs are unstable and may change without notice.

### Added

- Workspace split into a lightweight core plus feature-gated crates:
  - `ratatui-components-core`: theming, viewport/scroll, keymaps/help, selection, input primitives, TextArea, DataGrid, VirtualList, CodeView.
  - `ratatui-components-markdown`: MarkdownView (pulldown-cmark), Glow-inspired rendering options, optional `mdstream` integration.
  - `ratatui-components-syntax`: pluggable `CodeHighlighter` abstraction with feature-gated backends.
  - `ratatui-components`: facade crate with higher-level views (Diff, ANSI, Transcript) and re-exports.
- Rich content views and widgets:
  - `MarkdownView` (headings, lists, blockquotes, inline styles, code blocks; additional Glow parity features such as tables, images, footnotes).
  - `CodeView` (optional line numbers, selection, syntax highlighting via `CodeHighlighter`).
  - `DiffView` (unified diff rendering with hunk headers, +/- lines, optional intraline highlighting).
  - `AnsiTextView` (ANSI escape parsing to ratatui `Text`).
  - `TranscriptView` (agent-style transcript layout with mixed Markdown/ANSI/Diff content).
- Syntax highlighting backends:
  - Syntect backend (feature: `syntect`).
  - Tree-sitter backend (feature: `treesitter` + per-language features).
  - `AutoHighlighter` (prefer tree-sitter when available, otherwise fall back to syntect).
- Examples demonstrating the stack (see `docs/EXAMPLES.md`):
  - `preview` (Markdown + Diff + TextArea)
  - `transcript` / `agent_mvp` (transcript-style UIs)
  - `mdstream` (incremental streaming demo)
  - `code_ansi`, `dump`, `datagrid`, `virtual_list`

### Changed

- Highlighters expose a `highlight_text(...)` path to reduce per-frame allocations by batching lines.
- Tree-sitter highlighting is implemented via `tree-sitter-highlight` and uses the `0.26.x` tree-sitter API.
- TOML highlighting uses `tree-sitter-toml-ng` to avoid version conflicts.
- Markdown rendering rules are continuously aligned with Glow behavior; golden tests are used for parity checks.

### Removed

- Background/async highlighting workers; highlighting is synchronous on the main thread.

### Fixed

- Syntect color mapping improvements (including background handling and terminal profile support when enabled).

