# MVP Definition

## Goal

Ship a first version that enables building an AI-agent style TUI with ratatui, with a
focus on **rich content** (Markdown + code + diff + ANSI output) and **core input**
(multi-line prompt input).

## Intended Audience

- Developers building agent CLIs (chat + tools + patch review workflows).
- Developers needing a “docs viewer” experience inside a TUI.

## MVP Components (Must Have)

1. **Viewport / ScrollView**
   - Vertical scrolling with stable clamping.
   - Optional word-wrap for paragraph-like content.
   - Scrollbar rendering (optional).

2. **TextArea**
   - Multi-line editing with cursor, selection (optional for MVP), and paste support.
   - “Enter to submit / Shift+Enter for newline” style workflow (configurable).

3. **MarkdownView**
   - Basic Markdown: headings, paragraphs, lists, blockquotes, inline emphasis/strong/strike.
   - Code blocks supported (no tables/images required for MVP).
   - Rendering produces ratatui `Text` suitable for a viewport.

4. **CodeView + Syntax Highlighting (Feature-Gated)**
   - Highlighting backend is pluggable (see `docs/adr/0005-syntax-highlighting-backends.md`).
   - First backend: `syntect` (opt-in).
   - Fallback to plain code rendering when no backend is provided.

5. **DiffView**
   - Render unified diffs with hunk headers and +/- lines.
   - Default wrapping behavior: no soft-wrap for code/diff lines; use horizontal scrolling.
   - Optional intraline diff highlighting is a “nice-to-have” (can be added post-MVP).

6. **AnsiTextView**
   - Parse ANSI escape codes into ratatui `Text` for tool output panes.

## Non-Goals (MVP)

- Full Markdown spec coverage (tables, images, footnotes, HTML, etc.).
- A bundled runtime/event loop.
- Mouse interaction support (can be added later).
- Full-featured editor (LSP, undo/redo, etc.).

## API Expectations

- Backend-agnostic rendering (work with any ratatui backend).
- No forced global state; theming passed explicitly.
- Feature flags keep “core” lightweight:
  - `markdown` (parser)
  - `syntax-syntect` (syntect backend)
  - `syntax-treesitter` (tree-sitter backend, later)
  - `ansi` (ansi-to-tui)

## Acceptance Criteria

- Example app (in `examples/`) demonstrates:
  - A transcript pane rendering Markdown and ANSI output
  - A diff preview pane
  - A composer TextArea at the bottom
- Suggested: `cargo run -p ratatui-components --features transcript,syntect --example agent_mvp`
- Components do not panic on arbitrary UTF-8 input.
- Scrolling/wrapping remains stable when resizing the terminal.
- Syntax highlighting does not re-run on every frame (cache where applicable).
