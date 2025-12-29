use crate::view::MarkdownView;

#[deprecated(note = "Use MarkdownView instead (gives wrapping + scrolling + theming).")]
pub fn markdown_text(input: &str) -> ratatui::text::Text<'static> {
    let mut view = MarkdownView::new();
    view.set_markdown(input);
    view.as_text()
}
