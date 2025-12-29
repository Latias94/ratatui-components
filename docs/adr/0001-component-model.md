# ADR 0001: Component Model (Widget-first, component-friendly)

## Status

Proposed

## Context

Ratatui provides low-level `Widget`/`StatefulWidget` primitives. What’s missing in the ecosystem
is a consistent way to package **interactive, stateful UI pieces** (keybindings, scrolling, text
editing, etc.) without forcing a specific application runtime (tokio, async-std, sync loops).

We also want to make it easy to build Bubble Tea-like apps (update/render), but we should not
require it.

## Decision

Adopt a **widget-first** approach for the public API:

- Components expose a **state struct** plus a **render method** that draws into a `Buffer` or
  via ratatui’s `Frame`.
- Event handling is explicit: components provide `handle_event(...) -> Option<Action>` methods.
- Provide small adapters to support an Elm/Bubble Tea style `update` loop (optional, later).

Event types are defined in this crate (e.g. `InputEvent`) with feature-gated conversions from
`crossterm` events.

## Consequences

- Pros: integrates naturally into existing ratatui code; minimal coupling; easier adoption.
- Cons: more boilerplate than a full runtime; “commands” (async work) remain the app’s job.

## Alternatives Considered

1. **Runtime-first (Bubble Tea clone)**: simpler apps, but forces an opinionated loop and async model.
2. **Only `StatefulWidget`**: very idiomatic, but tends to scatter event handling and keymaps across apps.

