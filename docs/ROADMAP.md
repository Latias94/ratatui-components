# Roadmap

## Vision

`ratatui-components` provides **high-level, interactive components** that are:

- **Composable**: integrate into existing ratatui apps without forcing a runtime.
- **Batteries-included**: state + keybindings + helpers, not just drawing primitives.
- **Performant**: avoid per-frame heavy allocations; cache and invalidate predictably.
- **Themeable**: consistent “semantic tokens” for styling across all components.
- **Feature-gated**: keep the default dependency surface small.

## Packaging Strategy (Workspace)

Use a Cargo workspace to keep the “core” lightweight while allowing richer components to live in
separate crates:

- `ratatui-components` (core): viewport/scroll, keymaps, theming, basic list/input primitives
- `ratatui-components-markdown`: Markdown parsing/rendering
- `ratatui-components-diff`: unified diff parsing/rendering, optional intraline highlighting
- `ratatui-components-ansi`: ANSI → ratatui `Text`
- `ratatui-components-syntax-*`: optional syntax highlighting backends (e.g. syntect, tree-sitter)

## Target Use Cases

- **Agent CLIs**: chat transcript, streaming tool output, diff/patch preview, command palette.
- **Dev tools**: log viewers, file pickers, search results lists, diagnostics panes.
- **Docs viewers**: Markdown rendering, code blocks, syntax highlighting.

## Component Catalog (Proposed)

### Foundation (high leverage)

- **Viewport / ScrollView**: vertical scrolling, optional word-wrap, scrollbars, jump-to.
- **KeyMap + Help**: keybinding definitions and auto-rendered help line/panel.
- **TextInput / TextArea**: robust Unicode editing, paste bursts, submit semantics.
- **List**: selection, paging, optional fuzzy filtering and status line.

### Rich Content (differentiators)

- **MarkdownView**: headings, lists, blockquotes, inline styles, code blocks.
- **CodeView**: syntax highlighting (feature-gated), optional line numbers.
- **DiffView**: unified diff rendering, hunks, adds/removes, optional intraline diff.
- **AnsiTextView**: render ANSI-colored output to ratatui `Text`.

### Agent CLI Kit

- **TranscriptView**: role gutter, collapsible sections, streaming append, copy/select.
- **ToolCallView**: structured “tool call / tool result” blocks with status badges.
- **CommandPalette**: fuzzy command search, previews, keymap-driven.

### Utility

- **Toast / Banner**: ephemeral messages, error/success/info.
- **Modal**: centered overlay with focus trapping.
- **FilePicker / TreeView**: navigation for file systems and hierarchical data.

## Milestones

### M0 — Design (now)

- Define architecture: component model, event/action flow, theming, feature gating.
- Write initial ADRs and MVP acceptance criteria.

### M1 — MVP (ship something usable)

Deliver the minimal set that enables a “codex-like” CLI:

- Viewport/Scroll foundation
- TextArea (agent prompt input)
- MarkdownView (for assistant messages)
- DiffView + CodeView (patch review)
- AnsiTextView (tool output)
- Example app demonstrating the stack

See `docs/MVP.md`.

### M2 — Quality + Expansion

- CommandPalette + List (filtering/search)
- Better text shaping/wrapping and selection
- More examples and docs
- Theming polish and a default theme palette

### M3 — 1.0 Stabilization

- Public API stabilization and semantic versioning commitments
- Compatibility policy for ratatui versions
- Benchmarks for hot paths (wrapping, diff rendering, transcript append)
