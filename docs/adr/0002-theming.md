# ADR 0002: Theming via Semantic Tokens

## Status

Proposed

## Context

Ratatui styling is powerful but low-level (per-span, per-widget). For a components library, we
need consistent styling across many components and the ability for apps to apply a theme without
rewriting every render call.

Charmbracelet’s Lip Gloss popularizes a “style-first” approach; we want similar benefits while
staying idiomatic in Rust/ratatui.

## Decision

Introduce a `Theme` concept with **semantic tokens**, not per-component ad-hoc styles:

- `Theme` contains a palette (colors) and a set of styles (ratatui `Style`) keyed by semantics:
  - e.g. `text.primary`, `text.muted`, `accent`, `danger`, `code.inline`, `diff.add`, `diff.del`, ...
- Components accept `&Theme` (or a lightweight subset) and use tokens consistently.

The default theme is shipped by the crate; consumers can override tokens they care about.

## Consequences

- Pros: consistent look; easy global restyling; fewer “style params” per component.
- Cons: requires up-front token design; may not cover all edge cases (escape hatches still needed).

## Alternatives Considered

1. **Per-component style params everywhere**: flexible but verbose and inconsistent.
2. **A DSL like CSS**: expressive but heavy, harder to keep stable, and not idiomatic for ratatui.

