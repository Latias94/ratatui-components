use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::Span;

/// A pluggable syntax highlighter for code-like text.
///
/// This trait is used by multiple components:
/// - render cores (e.g. `code_render::render_code_lines`)
/// - interactive viewers (e.g. `CodeView`, `MarkdownView` code blocks, `DiffView`)
///
/// ## Contract
///
/// - `highlight_lines(language, lines)` must return `Vec<Vec<Span>>` with the **same length** as
///   `lines` (one span list per input line).
/// - Each span list is expected to represent the full line content in order. Components may patch
///   additional styles on top of your spans (e.g. theme base style, selection highlight).
/// - `Span::content` should be valid UTF-8 and should not contain newlines.
///
/// ## Performance notes
///
/// Many components call [`CodeHighlighter::highlight_text`] to batch-highlight a whole block in
/// one go. If you can implement `highlight_lines` efficiently (without per-line setup), you will
/// likely get good performance for free.
pub trait CodeHighlighter {
    /// Highlights `lines` and returns a span list for each input line.
    fn highlight_lines(&self, language: Option<&str>, lines: &[&str]) -> Vec<Vec<Span<'static>>>;

    /// Highlights `text` by splitting it on `\n` and forwarding to [`Self::highlight_lines`].
    fn highlight_text(&self, language: Option<&str>, text: &str) -> Vec<Vec<Span<'static>>> {
        let mut lines: Vec<&str> = text.split('\n').collect();
        if lines.is_empty() {
            lines.push("");
        }
        self.highlight_lines(language, &lines)
    }

    /// Optional background color hint for the highlighter.
    ///
    /// Components may use this to fill trailing cells (after the last glyph) for a more faithful
    /// code-block background.
    fn background_color(&self) -> Option<Color> {
        None
    }

    /// Convenience helper for highlighting a single line.
    fn highlight_line(&self, language: Option<&str>, line: &str) -> Vec<Span<'static>> {
        self.highlight_lines(language, &[line])
            .into_iter()
            .next()
            .unwrap_or_default()
    }
}

/// A no-op highlighter that returns unstyled spans.
pub struct NoHighlight;

impl CodeHighlighter for NoHighlight {
    fn highlight_lines(&self, _language: Option<&str>, lines: &[&str]) -> Vec<Vec<Span<'static>>> {
        lines
            .iter()
            .map(|l| vec![Span::styled((*l).to_string(), Style::default())])
            .collect()
    }
}
