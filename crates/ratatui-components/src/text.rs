use ratatui::style::Style;
use ratatui::text::Span;

pub trait CodeHighlighter {
    fn highlight_lines(&self, language: Option<&str>, lines: &[&str]) -> Vec<Vec<Span<'static>>>;

    fn highlight_line(&self, language: Option<&str>, line: &str) -> Vec<Span<'static>> {
        self.highlight_lines(language, &[line])
            .into_iter()
            .next()
            .unwrap_or_default()
    }
}

pub struct NoHighlight;

impl CodeHighlighter for NoHighlight {
    fn highlight_lines(&self, _language: Option<&str>, lines: &[&str]) -> Vec<Vec<Span<'static>>> {
        lines
            .iter()
            .map(|l| vec![Span::styled((*l).to_string(), Style::default())])
            .collect()
    }
}
