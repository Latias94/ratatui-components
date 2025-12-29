# MarkdownView ↔ Glow Parity Plan

This document tracks feature parity between `ratatui-components-markdown::MarkdownView` and
`glow` (repo-ref: `repo-ref/glow`) for practical, day-to-day Markdown documents.

Scope: focus on **readability and information completeness** first, then converge on **visual
polish**.

## Goals

- Match Glow’s common Markdown feature set (GFM-centric).
- Never lose information: unsupported constructs should degrade gracefully.
- Keep performance predictable for large documents (cache-by-width, avoid per-frame heavy work).

## Non-goals (for now)

- Pixel-perfect visual parity with Glow’s default theme.
- Mouse interactions, link opening, in-view search.
- Full HTML/CSS rendering.

## Current Status (Summary)

Implemented (baseline):

- Headings, paragraphs, blockquotes, ordered/unordered lists (nested), emphasis/strong/strike.
- Inline code and fenced/indented code blocks, with optional syntax highlighting (pluggable).
- GFM tables (basic layout), task lists, images (Glow-like `Image: Alt → URL`), horizontal rules.
- Tables default to a Glow-like style with box-drawing separators, with an optional full box-drawing style (`MarkdownViewOptions.table_style`).
- Footnote references and footnote definitions.
- Optional content padding via `MarkdownViewOptions.padding_left/padding_right` (useful to match Glow-like “inner margin”).
- Code blocks are indented like Glow (4 spaces; 2 spaces when nested in a blockquote).
- Relative links/images can be resolved via `MarkdownViewOptions.base_url` (similar to Glow `WithBaseURL`).

Known gaps:

- Tight/loose list spacing parity with Glow.
- Table rendering polish (multi-line cell edge cases, alignment edge cases).
- Link rendering policy details (reference-style links, titles); destination URL can be shown with `MarkdownViewOptions.show_link_destinations`.
- HTML and math are placeholders (rendered as muted/raw-ish text).

## Parity Checklist

Legend:
- P0: required for “complete MarkdownView”
- P1: strongly recommended
- P2: optional / later

### Core Markdown (P0)

- [x] Headings (H1–H6)
- [x] Paragraphs + word wrap
- [x] Emphasis / strong / strikethrough
- [x] Inline code
- [x] Blockquotes (with nested indentation)
- [x] Ordered + unordered lists (nested indentation)
- [x] Hard breaks vs soft breaks
- [x] Horizontal rule
- [x] Code blocks (fenced + indented)

### GFM Extensions (P0)

- [x] Task list items (`- [ ]`, `- [x]`)
- [x] Tables (basic)
- [x] Footnotes (references + definitions)

### Links / Images (P0)

- [x] Link text styling
- [x] Link destination display policy (optional via `MarkdownViewOptions.show_link_destinations`)
- [x] Image fallback rendering (`alt (url)`)

### Visual Parity (P1)

- [ ] Consistent list spacing (tight vs loose) matching Glow
- [ ] Table borders/separators closer to Glow
- [ ] Code block “frame” / background feel (token-based, themeable)
- [ ] Better heading typography (tokens per level)
- [ ] Blockquote styling (bar + muted text)

Notes:

- Glow renders task list markers as `[✓]` / `[ ]`.
- Glow renders horizontal rules as `--------` (8 hyphens), independent of terminal width.
- Glow renders images as `Image: Alt → URL` and lets wrapping split the URL across lines.
- Glow has a quirk for lists inside blockquotes: when a list item wraps, continuation lines may not repeat the `| ` prefix.

### Rich Content (P2)

- [x] Inline HTML strategy (strip tags, keep text; rendered muted)
- [ ] Math rendering strategy (keep-as-code vs real layout)
- [ ] Definition lists / metadata blocks (if needed)

## Fixture-Driven Alignment

Use the fixture file as a “contract” for parity work:

- `docs/fixtures/glow_parity.md`

Snapshot-driven, step-by-step tracking:

- `docs/PARITY_MARKDOWNVIEW_GLOW.md`

Workflow per item:

1. Add (or extend) fixture section that demonstrates the feature.
2. Render the fixture in our preview app and in Glow.
3. Decide the expected behavior (information-preserving + readable).
4. Add/adjust unit tests for `MarkdownView` for the minimal invariants (no panic + key substrings).

## Manual Verification Steps

1. Run our preview app:
   - `cargo run -p ratatui-components --example preview`
2. Dump a plain-text render (useful for diffing against Glow output):
   - `cargo run -p ratatui-components-markdown --example dump -- --width 80 docs/fixtures/glow_parity.md`
   - (Optional) with base URL: `cargo run -p ratatui-components-markdown --example dump -- --width 80 --show-link-destinations --base-url https://example.com/docs/ docs/fixtures/glow_parity.md`
3. (Optional) Render with Glow for comparison (outside of Rust build):
   - `glow docs/fixtures/glow_parity.md`
