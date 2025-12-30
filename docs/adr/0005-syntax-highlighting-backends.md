# ADR 0005: Pluggable Syntax Highlighting (Syntect first, Tree-sitter later)

## Status

Accepted

## Context

Syntax highlighting materially improves CodeView and DiffView. Zed demonstrates the power of
tree-sitter-based highlighting, but tree-sitter comes with operational complexity:

- Language grammar management (per-language crates, versions, builds)
- Capture query mapping to styles
- Higher overall dependency weight

Syntect (TextMate grammars) is easier to integrate and covers many languages with less plumbing,
but is less structurally precise than tree-sitter.

## Decision

Define a small, stable highlighting abstraction in the ecosystem and keep the backend pluggable:

- Core defines a trait (e.g. `CodeHighlighter`) that converts `(language, source)` into styled
  lines/spans.
- Default behavior is “no highlighting” (plain code rendering).
- Provide `syntect` backend as the first optional implementation (ship earlier, lower integration cost).
- Add a tree-sitter backend later behind an opt-in Cargo feature (e.g. `treesitter`).

DiffView uses the same abstraction for context lines and intraline highlights (when enabled).

For the tree-sitter backend, expose **per-language Cargo features** so consumers opt into only what
they need:

- `syntax-treesitter` enables the backend infrastructure (no languages by default).
- `syntax-treesitter-lang-rust`, `syntax-treesitter-lang-python`, ... enable optional dependencies on
  the corresponding `tree-sitter-*` grammar crates and register them.
- Provide convenience “packs” (non-default), e.g. `syntax-treesitter-langs-default` or
  `syntax-treesitter-langs-all`, to reduce boilerplate for users who want many languages.

Also support manual registration so users can bring their own grammars without waiting for this
ecosystem to vendor them.

## Consequences

- Pros: users choose trade-offs; we can ship MVP without committing to one “forever” backend.
- Cons: a trait boundary limits some advanced backend-specific features; needs careful API design.

## Alternatives Considered

1. Tree-sitter only: best quality long-term, but too heavy/complex for early iterations.
2. Syntect only: simplest, but may not satisfy “editor-grade” expectations over time.
