# ADR 0003: Rich Text Pipeline (Markdown / ANSI / Wrap)

## Status

Proposed

## Context

The “killer features” (Markdown rendering, code blocks, diffs, ANSI tool output) all share the
same hard problems:

- Correct width measurement (Unicode width, tabs)
- Wrapping without breaking styles
- Caching to avoid heavy work every frame
- Stable scrolling when geometry changes

We need a common pipeline so each component doesn’t reinvent wrapping and scrolling.

## Decision

Build a shared “rich text” layer that produces ratatui `Text` plus metadata needed for scrolling:

- Normalize input (tabs, line endings).
- Convert source formats into styled lines:
  - Markdown via `pulldown-cmark` (feature `markdown`)
  - ANSI via `ansi-to-tui` (feature `ansi`)
  - Diff via a simple parser (feature `diff`)
- Wrap at a given width with caching (recompute only when width or content changes).
- Viewport renders a window over the wrapped lines.

Syntax highlighting for code blocks is optional and uses pluggable backends (see
`docs/adr/0005-syntax-highlighting-backends.md`).

Wrapping policy by content type:

- Prose (Markdown paragraphs, blockquotes, list items): word-wrap by default.
- Code-like content (code blocks, diffs): no soft-wrap by default; prefer horizontal scrolling.
  A soft-wrap mode may be offered as an opt-in rendering option.

## Consequences

- Pros: shared correctness/perf; consistent wrapping/scrolling behavior across views.
- Cons: wrapping styled content is complex; may require iterative improvements post-MVP.

## Alternatives Considered

1. **Let each component do its own wrapping**: fast to prototype, but inconsistent and bug-prone.
2. **No wrapping, only horizontal scroll**: simpler, but Markdown paragraphs become unpleasant to read.
