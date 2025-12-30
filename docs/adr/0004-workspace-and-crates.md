# ADR 0004: Workspace + Crate Split for Dependency Isolation

## Status

Accepted (revised)

## Context

We want a “general-purpose components ecosystem” for ratatui. Some components are lightweight
(viewport, keymaps, basic input), while others naturally pull heavier dependencies (Markdown
parsing, syntax highlighting, tree-sitter grammars).

If everything lives in one crate, users pay higher compile times and dependency footprint even
when they only need small pieces.

## Decision

Use a Cargo workspace with a lightweight core crate, and keep heavier domains (Markdown, syntax
highlighting backends) in opt-in crates. Provide a facade crate (`ratatui-components`) for a
single-dependency UX via re-exports.

## Consequences

- Pros: simpler packaging and API surface; fewer moving parts for examples/docs; still keeps default deps small.
- Cons: feature-flag matrix can grow; internal module boundaries require discipline.

## Alternatives Considered

1. Single crate with feature flags only: simpler packaging but still tends to centralize heavy deps.
2. `ratatui-components-core` + `ratatui-components` (facade): nicer UX but adds an extra layer early.
