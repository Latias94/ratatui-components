# Streaming (Agent CLI Patterns)

This repository targets “agent-style” TUIs where assistant/tool output arrives as small deltas.
Rendering on every token can cause visible stutter due to repeated parsing, wrapping, and syntax
highlighting.

## Recommended Pattern

- Keep your UI state on a single thread.
- Send deltas from background tasks to the UI thread over a channel.
- Coalesce deltas before updating the UI:
  - Prefer “newline-gated” flush (commit when a `\n` arrives).
  - Add a time-based fallback (e.g. flush every 30–100ms).

## TranscriptView

`TranscriptView` is designed for chat-like transcripts with a role gutter and mixed content:
Markdown, diffs, ANSI output, and plain text.

For streaming assistant messages, update the last entry instead of pushing a new entry per token:

- `TranscriptView::append_to_last_markdown(Role::Assistant, delta)`
- `TranscriptView::push_or_append_markdown(Role::Assistant, delta)` (convenience)

The view keeps a per-entry render cache and only invalidates the changed entry on append.

If you enable the `mdstream` feature on `ratatui-components-transcript`, the view also keeps an
incremental renderer for the currently streaming Markdown entry (committed blocks cached, pending
tail updated). This reduces stutter when the last assistant message grows large, especially with
open code fences.

By default, open (pending) code fences are truncated while streaming (last ~40 lines) to avoid
stuttering on large outputs. The full code block is shown once the fence is closed.

You can tweak or disable this:

- `TranscriptViewOptions.streaming_markdown_pending_code_fence_max_lines`
- `TranscriptView::set_streaming_markdown_pending_code_fence_max_lines(None)`

Run the example with the feature enabled:

- `cargo run -p ratatui-components-transcript --features mdstream --example transcript`

## MarkdownView

`MarkdownView` is optimized for interactive rendering (cache-by-width + background highlighting).
When `set_markdown()` is called repeatedly, it preserves the syntax highlight cache so stable code
blocks are not re-highlighted from scratch.

For best UX while streaming, still avoid calling `set_markdown()` on every tiny delta; apply the
coalescing strategy above.

If you want to avoid re-parsing / re-layouting the entire document on each flush, use
`ratatui-components-markdown` with the `mdstream` feature and render via `MarkdownStreamView`
(incremental: committed blocks are rendered once, pending tail is updated):

- `ratatui_components_markdown::streaming::MarkdownStreamView`
- Optional: `MarkdownStreamView::set_pending_code_fence_max_lines(Some(n))` to truncate very large
  pending code fences (Gemini CLI style) and reduce stutter.
