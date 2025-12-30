use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::Span;

pub trait CodeHighlighter {
    fn highlight_lines(&self, language: Option<&str>, lines: &[&str]) -> Vec<Vec<Span<'static>>>;

    fn highlight_text(&self, language: Option<&str>, text: &str) -> Vec<Vec<Span<'static>>> {
        let mut lines: Vec<&str> = text.split('\n').collect();
        if lines.is_empty() {
            lines.push("");
        }
        self.highlight_lines(language, &lines)
    }

    fn background_color(&self) -> Option<Color> {
        None
    }

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
