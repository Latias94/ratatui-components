# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
once it is released.

## [0.1.0] - Unreleased

This project has not been released yet. APIs are unstable and may change without notice.

`ratatui-components` provides batteries-included, themeable building blocks for modern TUIs built
with `ratatui`, inspired by Charmbracelet’s ecosystem (Glow/Bubbles/Lip Gloss).

### Highlights

- No runtime: components are event-loop agnostic and integrate into existing ratatui apps.
- Feature-gated heavy deps: keep core lightweight; opt into markdown/syntax/ansi as needed.
- Built for real apps: scrolling, selection + copy, stable rendering, and caching.

### Added

- Workspace split into a lightweight core plus feature-gated crates:
  - `ratatui-components-core`: theming, viewport/scroll, keymaps/help, selection, input primitives, TextArea, DataGrid, VirtualList, CodeView.
  - `ratatui-components-markdown`: MarkdownView (pulldown-cmark), Glow-inspired rendering options, optional `mdstream` integration.
  - `ratatui-components-syntax`: pluggable `CodeHighlighter` abstraction with feature-gated backends.
  - `ratatui-components`: facade crate with higher-level views (Diff, ANSI, Transcript) and re-exports.
- Core building blocks (in `ratatui-components-core`):
  - Theming (`Theme`) with semantic tokens.
  - Viewport state (`ViewportState`) + scroll bindings (`ScrollBindings`) and scrollbar rendering.
  - Keymap helpers + `HelpBar`.
  - Selection model + `SelectionBindings` (mouse drag selection + copy-on-request).
  - `TextArea` (multi-line editing, paste handling, submit semantics).
  - `DataGrid` and `VirtualList`.
  - `CodeView` (scrollable code viewer with optional line numbers + selection).
- Rich content views (in `ratatui-components` facade):
  - `MarkdownView` (feature: `markdown`): Glow-inspired Markdown rendering (headings, lists, quotes, code blocks, tables, images, footnotes, task lists, link destination policies).
  - `DiffView` (feature: `diff`): unified diff rendering with hunks, +/- lines, optional intraline change highlighting.
  - `AnsiTextView` (feature: `ansi`): ANSI escape parsing to ratatui text.
  - `TranscriptView` (feature: `transcript`): agent-style transcript with role gutter and mixed content.
- Render cores for custom layouts (no viewport/selection included):
  - `markdown::document::MarkdownDocument` (feature: `markdown`): parse once, render to `Text` for any layout/virtualizer.
  - `code_render::render_code_lines` (core): render code lines to `Text` with optional line numbers + optional highlighting.
- Syntax highlighting backends:
  - Syntect backend (feature: `syntect`).
  - Tree-sitter backend (feature: `treesitter` + per-language features, plus bundles like `treesitter-langs-common`).
  - `AutoHighlighter` (prefer tree-sitter when available, otherwise fall back to syntect).
- Examples demonstrating the stack (see `docs/EXAMPLES.md`):
  - `preview` (Markdown + Diff + TextArea)
  - `transcript` / `agent_mvp` (transcript-style UIs)
  - `mdstream` (incremental streaming demo)
  - `code_ansi`, `dump`, `datagrid`, `virtual_list`
- Release engineering helpers:
  - `scripts/release-check.zsh` to validate formatting, tests, feature matrix, and example builds.

### Changed

- Highlighters expose a `highlight_text(...)` path to reduce per-frame allocations by batching lines.
- Tree-sitter highlighting is implemented via `tree-sitter-highlight` and uses the `0.26.x` tree-sitter API.
- TOML highlighting uses `tree-sitter-toml-ng` to avoid version conflicts.
- Markdown rendering rules are continuously aligned with Glow behavior; golden tests are used for parity checks.
- Mouse-driven selection is clamped to the visible content area to support drag-outside behavior.

### Removed

- Background/async highlighting workers; highlighting is synchronous on the main thread.

### Fixed

- Syntect color mapping improvements (including background handling and terminal profile support when enabled).
