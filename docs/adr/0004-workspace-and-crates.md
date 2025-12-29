# ADR 0004: Workspace + Crate Split for Dependency Isolation

## Status

Accepted

## Context

We want a “general-purpose components ecosystem” for ratatui. Some components are lightweight
(viewport, keymaps, basic input), while others naturally pull heavier dependencies (Markdown
parsing, syntax highlighting, tree-sitter grammars).

If everything lives in one crate, users pay higher compile times and dependency footprint even
when they only need small pieces.

## Decision

Use a Cargo **workspace** and split by dependency weight:

- `ratatui-components` (core): foundational components and shared abstractions (Theme, wrapping,
  scrolling, keymaps).
- `ratatui-components-markdown`: Markdown → styled lines/text, built on `pulldown-cmark`.
- `ratatui-components-diff`: unified diff parsing/rendering and diff-specific UI helpers.
- `ratatui-components-ansi`: ANSI escape parsing into ratatui `Text`.
- `ratatui-components-syntax-*`: opt-in syntax highlighting backends.

Each crate can still expose feature flags for further optional capabilities, but “heavy” should
not be in `ratatui-components` by default.

## Consequences

- Pros: smaller default footprint; clearer ownership boundaries; faster builds for core users.
- Cons: more crates to document/release; version coordination across crates.

## Alternatives Considered

1. Single crate with feature flags only: simpler packaging but still tends to centralize heavy deps.
2. `ratatui-components-core` + `ratatui-components` (facade): nicer UX but adds an extra layer early.
