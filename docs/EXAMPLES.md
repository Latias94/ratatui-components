# Examples

## Render Core (MarkdownDocument)

If you want full control over layout (multi-pane UIs, custom scrolling, virtualization), use the
rendering core directly and render the returned `Text` wherever you want:

```rust
use ratatui::text::Text;
use ratatui_components_core::theme::Theme;
use ratatui_components_markdown::document::{MarkdownDocument, MarkdownRenderOptions};

let opts = MarkdownRenderOptions::default();
let doc = MarkdownDocument::parse("# Title\n\nHello **world**.\n", &opts);
let rendered = doc.render(80, &Theme::default(), &opts, None);
let text: Text<'static> = rendered.into_text();
```

## Render Core (Code)

If you want to render code with optional syntax highlighting, but handle layout/scrolling yourself:

```rust
use ratatui_components_core::code_render::{
    render_code_lines, CodeRenderOptions, CodeRenderStyles,
};
use ratatui_components_core::theme::Theme;

let theme = Theme::default();
let code = ["fn main() {", "  println!(\"hi\");", "}"];

let rendered = render_code_lines(
    &code,
    Some("rs"),
    None, // or Some(&*highlighter)
    CodeRenderStyles {
        base: theme.code_inline,
        gutter: theme.text_muted,
    },
    CodeRenderOptions {
        show_line_numbers: true,
        ..Default::default()
    },
);
let text = rendered.into_text();
```

## Preview (MarkdownView + DiffView + TextArea)

Run:

`cargo run -p ratatui-components --features diff,markdown,syntect --example preview`

Keys:

- `Tab`: switch focus
- `j/k` or `↑/↓`: scroll
- `h/l` or `←/→`: horizontal scroll (Diff / code)
- `g/G`: top/bottom
- `q`: quit

Notes:

- `MarkdownView` can show code line numbers via `MarkdownViewOptions.show_code_line_numbers`.

## CodeView + AnsiTextView

Run:

`cargo run -p ratatui-components --features ansi,syntect --example code_ansi`

Notes:

- `MarkdownView`, `CodeView`, `DiffView`, and `AnsiTextView` expose `selected_text()` and mouse-driven selection helpers via `handle_event_action_in_area(...)` (experimental).
- If you enable mouse capture/reporting, your terminal’s native copy-by-selection usually won’t work; call `selected_text()` and copy it in the app (manual copy action, not automatic).

Keys:

- `Tab`: switch focus
- `j/k` or `↑/↓`: scroll
- `h/l` or `←/→`: horizontal scroll
- `g/G`: top/bottom
- `q`: quit

## TranscriptView (agent-style transcript)

Run:

`cargo run -p ratatui-components --features transcript,syntect --example transcript`

MVP-style layout (transcript + diff + composer):

`cargo run -p ratatui-components --features transcript,syntect --example agent_mvp`

Optional (incremental streaming markdown via `mdstream`):

`cargo run -p ratatui-components --features transcript,mdstream,syntect --example transcript`

Keys:

- `Tab`: switch focus
- `j/k` or `↑/↓`: scroll transcript
- `Ctrl+u / Ctrl+d`: page up/down
- `y`: request copy of current selection
- `Esc`: clear selection
- `f`: toggle follow-tail
- `q`: quit

## mdstream + MarkdownView (streaming demo)

Run:

`cargo run -p ratatui-components --features mdstream,syntect --example mdstream`

Notes:

- Left pane uses repeated `MarkdownView::set_markdown()` on the raw string.
- Right pane uses `MarkdownStreamView` (incremental: committed blocks cached, pending tail updated).

## render_core (custom layout using render cores)

Run:

`cargo run -p ratatui-components --features markdown --example render_core`

Optional (syntax highlighting via syntect):

`cargo run -p ratatui-components --features markdown,syntect --example render_core`
