use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use unicode_width::UnicodeWidthStr;

use crate::text::CodeHighlighter;

/// Styling and layout options for the `render_code_lines` render core.
///
/// This type is intentionally small and copyable so apps can cache rendered output keyed by:
/// - the input lines (or a hash of the source text)
/// - `language`
/// - `styles` + `options`
#[derive(Clone, Debug)]
pub struct CodeRenderOptions {
    /// Whether to show 1-based line numbers.
    pub show_line_numbers: bool,
    /// The line number of the first rendered line (1-based).
    pub line_number_start: usize,
    /// Separator after the line number gutter (e.g. `" │ "`).
    pub line_number_separator: &'static str,
}

impl Default for CodeRenderOptions {
    fn default() -> Self {
        Self {
            show_line_numbers: false,
            line_number_start: 1,
            line_number_separator: " │ ",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CodeRenderStyles {
    /// Base style applied to code spans (patched onto highlight styles).
    pub base: Style,
    /// Style used for the line number gutter.
    pub gutter: Style,
}

impl Default for CodeRenderStyles {
    fn default() -> Self {
        Self {
            base: Style::default(),
            gutter: Style::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct RenderedCode {
    /// Fully materialized lines ready for drawing via `Paragraph`/custom rendering.
    pub lines: Vec<Line<'static>>,
    /// Maximum display width (in terminal cell units) across all rendered lines.
    pub content_width: u32,
    /// Total number of rendered lines.
    pub content_height: u32,
}

impl RenderedCode {
    pub fn into_text(self) -> Text<'static> {
        Text::from(self.lines)
    }
}

/// Renders code lines into styled [`Line`]s, optionally using a [`CodeHighlighter`].
///
/// This is a rendering core. It intentionally does not include viewport state, selection,
/// scrolling, or hit-testing.
///
/// # Caching
///
/// This function allocates and returns an owned `Vec<Line<'static>>`. For typical TUIs, it is
/// recommended to cache the returned [`RenderedCode`] and only re-render when:
/// - the source lines change
/// - you toggle line numbers / change the gutter formatting
/// - your theme or highlighter changes
///
/// If a highlighter is provided, the implementation calls `highlight_text(...)` once for the whole
/// input (joined with `\n`) and then slices the output back into per-line spans.
pub fn render_code_lines<S: AsRef<str>>(
    lines: &[S],
    language: Option<&str>,
    highlighter: Option<&dyn CodeHighlighter>,
    styles: CodeRenderStyles,
    options: CodeRenderOptions,
) -> RenderedCode {
    let line_number_w = if options.show_line_numbers {
        digits(
            options
                .line_number_start
                .saturating_add(lines.len().saturating_sub(1)),
        )
        .max(1)
    } else {
        0
    };

    let highlighted = highlighter.map(|hi| {
        let mut text = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                text.push('\n');
            }
            text.push_str(line.as_ref());
        }
        hi.highlight_text(language, &text)
    });

    let mut out: Vec<Line<'static>> = Vec::with_capacity(lines.len().max(1));
    let mut max_w = 0u32;

    if lines.is_empty() {
        out.push(Line::from(vec![Span::styled(String::new(), styles.base)]));
        return RenderedCode {
            lines: out,
            content_width: 0,
            content_height: 1,
        };
    }

    for (idx, raw) in lines.iter().enumerate() {
        let mut spans: Vec<Span<'static>> = Vec::new();

        if options.show_line_numbers {
            let n = options.line_number_start.saturating_add(idx);
            let gutter = format!(
                "{n:>width$}{}",
                options.line_number_separator,
                width = line_number_w
            );
            spans.push(Span::styled(gutter, styles.gutter));
        }

        let mut code_spans = highlighted
            .as_ref()
            .and_then(|h| h.get(idx).cloned())
            .unwrap_or_else(|| vec![Span::styled(raw.as_ref().to_string(), styles.base)]);

        for s in &mut code_spans {
            s.style = styles.base.patch(s.style);
        }
        spans.extend(code_spans);

        let plain = join_spans_plain(&spans);
        max_w = max_w.max(UnicodeWidthStr::width(plain.as_str()) as u32);
        out.push(Line::from(spans));
    }

    RenderedCode {
        content_width: max_w,
        content_height: out.len() as u32,
        lines: out,
    }
}

fn join_spans_plain(spans: &[Span<'static>]) -> String {
    let mut out = String::new();
    for s in spans {
        out.push_str(s.content.as_ref());
    }
    out
}

fn digits(mut n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    let mut d = 0;
    while n > 0 {
        n /= 10;
        d += 1;
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;
    use ratatui::style::Style;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    #[test]
    fn uses_highlight_text_once_for_all_lines() {
        #[derive(Default)]
        struct CountingHighlighter {
            calls: AtomicUsize,
        }

        impl CodeHighlighter for CountingHighlighter {
            fn highlight_lines(
                &self,
                _language: Option<&str>,
                lines: &[&str],
            ) -> Vec<Vec<Span<'static>>> {
                self.calls.fetch_add(1, Ordering::SeqCst);
                lines
                    .iter()
                    .map(|l| vec![Span::styled((*l).to_string(), Style::default())])
                    .collect()
            }
        }

        let hi = CountingHighlighter::default();
        let _ = render_code_lines(
            &["a", "b", "c"],
            Some("rs"),
            Some(&hi),
            CodeRenderStyles::default(),
            CodeRenderOptions::default(),
        );

        assert_eq!(hi.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn renders_line_numbers_with_separator() {
        let rendered = render_code_lines(
            &["a", "b"],
            None,
            None,
            CodeRenderStyles {
                base: Style::default(),
                gutter: Style::default().fg(Color::Red),
            },
            CodeRenderOptions {
                show_line_numbers: true,
                line_number_start: 10,
                line_number_separator: " | ",
            },
        );

        assert_eq!(rendered.content_height, 2);
        assert!(
            rendered
                .lines
                .first()
                .expect("line exists")
                .spans
                .first()
                .expect("span exists")
                .content
                .as_ref()
                .contains("10 | ")
        );
    }
}
