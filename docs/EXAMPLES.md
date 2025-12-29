# Examples

## Preview (MarkdownView + DiffView + TextArea)

Run:

`cargo run -p ratatui-components --example preview`

Keys:

- `Tab`: switch focus
- `j/k` or `↑/↓`: scroll
- `h/l` or `←/→`: horizontal scroll (Diff / code)
- `g/G`: top/bottom
- `q`: quit

Notes:

- `MarkdownView` can show code line numbers via `MarkdownViewOptions.show_code_line_numbers`.

## TranscriptView (agent-style transcript)

Run:

`cargo run -p ratatui-components-transcript --example transcript`

Optional (incremental streaming markdown via `mdstream`):

`cargo run -p ratatui-components-transcript --features mdstream --example transcript`

Keys:

- `Tab`: switch focus
- `j/k` or `↑/↓`: scroll transcript
- `Ctrl+u / Ctrl+d`: page up/down
- `f`: toggle follow-tail
- `q`: quit

## mdstream + MarkdownView (streaming demo)

Run:

`cargo run -p ratatui-components-markdown --features mdstream --example mdstream`

Notes:

- Left pane uses repeated `MarkdownView::set_markdown()` on the raw string.
- Right pane uses `MarkdownStreamView` (incremental: committed blocks cached, pending tail updated).
