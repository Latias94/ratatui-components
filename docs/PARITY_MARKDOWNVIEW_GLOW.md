# MarkdownView ↔ Glow Parity (Step-by-step)

This document is the execution plan for aligning `ratatui-components-markdown::MarkdownView`
to Charmbracelet `glow` (repo-ref: `repo-ref/glow`) using snapshot (“golden”) outputs.

The goal is **feature parity and readable layout parity** for day-to-day Markdown documents.
Pixel-perfect styling parity is explicitly out of scope for now.

## Golden Workflow

We keep two sets of snapshots:

- `docs/fixtures/golden/ratatui/*.txt`: our renderer output (authoritative for regressions).
- `docs/fixtures/golden/glow/*.txt`: reference output from `glow` (used for manual comparison).

Generate/update both:

- `scripts/gen-goldens.zsh`

Diff the two snapshot sets:

- `scripts/diff-glow.zsh`

Note: the Glow snapshots are generated from `repo-ref/glow` in CLI mode with a pinned style
(`-s dracula`) to keep output stable, then normalized (strip ANSI + trim trailing whitespace +
strip the common left margin).

Update only our snapshots (when behavior changes intentionally):

- `UPDATE_GOLDENS=1 cargo test -p ratatui-components-markdown golden`

## What We Compare (and What We Don’t)

Important: the snapshots are **plain text** (styles are not represented). Some Glow behaviors are
implemented via styling rather than literal characters (e.g. heading “#” markers). We treat those
as “reference hints”, not hard requirements, unless we decide otherwise.

Parity target per feature is expressed as **invariants** (what must be true in the output), so
we can converge without getting blocked on style differences.

## Current Fixture

- Fixture: `docs/fixtures/glow_parity.md`
- Ratatui snapshots:
  - `docs/fixtures/golden/ratatui/glow_parity__glow_like__w80.txt`
  - `docs/fixtures/golden/ratatui/glow_parity__glow_like__w40.txt`
- Glow snapshots:
  - `docs/fixtures/golden/glow/glow_parity__glow_like__w80.txt`
  - `docs/fixtures/golden/glow/glow_parity__glow_like__w40.txt`

Options used on the Ratatui side (see `crates/ratatui-components-markdown/tests/golden.rs`):

- `preserve_new_lines = true`
- `show_link_destinations = true`
- `link_destination_style = Space`
- `show_heading_markers = true`
- `glow_compat_relative_paths = true`
- `padding_left = 2`
- `padding_right = 2`
- `blockquote_prefix = "  "` (two spaces)
- `table_style = Glow`
- `glow_compat_quote_list_wrap = true`
- `code_block_indent = 2`
- `code_block_indent_in_blockquote = 2`
- `footnote_hanging_indent = false`
- `glow_compat_loose_list_join = true`
- `glow_compat_post_list_blank_lines = 3`

## Step-by-step Parity Checklist

Each item below has:

- **Invariants**: what we must guarantee.
- **Glow reference**: how Glow behaves today (for eyeballing).
- **Status**: `match` / `partial` / `mismatch`.

### 1) Headings

Invariants:

- H1–H6 are visually distinct in the TUI (style/token), and keep the heading text intact.
- Heading boundaries don’t collapse into adjacent paragraphs.

Glow reference:

- Glow includes `#`/`##` markers in its plain output snapshot.

Status: `match` (when `show_heading_markers = true` in the parity profile).

### 2) Inline styles (em/strong/strike/inline code)

Invariants:

- Inline formatting does not lose text.
- Inline code keeps its content verbatim and does not wrap “strangely” compared to prose.

Status: `partial` (plain output drops style markers, expected).

### 3) Links (autolink, inline link, reference link)

Invariants:

- Link text is preserved.
- When `show_link_destinations = true`, destination is shown for:
  - inline links (`[text](url)`)
  - reference links (`[text][ref]`)
  - autolinks (`<url>`) should render as `url`
- Destination display policy is stable (don’t show redundant `(url)` if text already equals url).

Glow reference:

- Glow prints `text url` (space-separated) in its plain output.

Status: `match` (when `link_destination_style = Space` in the parity profile).

### 4) Lists (nested, ordered)

Invariants:

- Bullet list marker is stable (`•`) and nested indentation is consistent.
- Ordered list numbering is correct.
- Wrapped list continuation lines align under the list text (not under the bullet).

Status: `match` (for the basic cases in the fixture).

### 5) Loose list spacing (paragraphs inside list items)

Invariants:

- A second paragraph in a list item is indented (not re-bulleted).
- Spacing is consistent and does not eat newlines unexpectedly.

Glow reference:

- Glow has a quirk where “item A” and its second paragraph can appear without a newline in
  the plain output snapshot (likely renderer-specific).

Status: `match` (when `glow_compat_loose_list_join = true` in the parity profile; default is more readable).

### 6) Task list items

Invariants:

- `- [x]` and `- [ ]` render as `[✓]` and `[ ]` (no bullet).
- There is at least one space between the marker and the task text.
- Marker is aligned with other list markers.

Status: `match` (for the current fixture coverage).

### 7) Blockquote (including list + code inside quote)

Invariants:

- Quote prefix is stable (and configurable via `MarkdownViewOptions.blockquote_prefix`).
- Nested quote content keeps indentation and does not lose structure.
- With `glow_compat_quote_list_wrap = true`, quote markers should not “bleed” into wrapped list
  continuation lines.

Status: `match` (for current fixture coverage).

### 8) Fenced code blocks

Invariants:

- Fenced code blocks render with a stable indentation and preserve code content.
- Language hint normalization is consistent across common syntaxes (` ```rs`, ` ```language-rust`, ` ```{.rust}`).

Status: `match` (basic cases covered).

### 9) Tables (Glow-style)

Invariants:

- Column alignment: `:---` / `---:` / `:---:` behave as left/right/center.
- Multi-line cell wrapping keeps column boundaries aligned.
- Header separator length is stable and does not jitter with content.

Glow reference:

- Glow uses box-drawing separators (`│` and `┼`) and a box-drawing rule (`─`) for the header separator.

Status: `match` (for the current fixture coverage).

### 10) Images

Invariants:

- Render as `Image: ALT → URL` (ALT defaults to `[image]`).
- When `base_url` is provided, relative URLs are resolved.
- Long URLs can wrap without losing characters.

Status: `match` (with `base_url` off; with `base_url` on is covered elsewhere).

### 11) Footnotes

Invariants:

- References remain in text.
- Definitions render with stable indentation and wrap correctly.
- Optional: when `footnotes_at_end = true`, definitions are moved to the end.

Status: `match` (basic fixture).

### 12) Horizontal rule

Invariants:

- Render as exactly 8 hyphens (`--------`), independent of width.

Status: `match`.

### 13) HTML + Math (fallback)

Invariants:

- Do not panic.
- Preserve user-visible content as much as possible (strip tags / keep raw).

Status: `partial` (fallback behavior differs between renderers; we treat this as non-blocking).

## Next Steps (Recommended Order)

1. Decide whether to emulate Glow’s loose-list quirk in a dedicated compat option.
2. Expand the fixture set (tables edge-cases, nested quotes, long links, code fence variants).
