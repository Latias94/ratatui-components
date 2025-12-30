use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui_components_core::text::CodeHighlighter;
use syntect::easy::HighlightLines;
use syntect::highlighting::FontStyle;
use syntect::highlighting::Style as SynStyle;
use syntect::highlighting::Theme;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxReference;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

pub struct SyntectHighlighter {
    syntax_set: SyntaxSet,
    theme: Theme,
}

impl SyntectHighlighter {
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        let theme = theme_set
            .themes
            .get("base16-ocean.dark")
            .cloned()
            .or_else(|| theme_set.themes.values().next().cloned())
            .unwrap_or_default();
        Self { syntax_set, theme }
    }

    fn syntax_for(&self, language: Option<&str>) -> &SyntaxReference {
        if let Some(lang) = language {
            if let Some(syntax) = self.syntax_set.find_syntax_by_extension(lang) {
                return syntax;
            }
            if let Some(syntax) = self.syntax_set.find_syntax_by_token(lang) {
                return syntax;
            }
        }
        self.syntax_set.find_syntax_plain_text()
    }
}

impl Default for SyntectHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeHighlighter for SyntectHighlighter {
    fn highlight_lines(&self, language: Option<&str>, lines: &[&str]) -> Vec<Vec<Span<'static>>> {
        let syntax = self.syntax_for(language);
        let mut highlighter = HighlightLines::new(syntax, &self.theme);

        let mut out: Vec<Vec<Span<'static>>> = Vec::with_capacity(lines.len());
        for line in lines {
            let mut spans: Vec<Span<'static>> = Vec::new();
            for l in LinesWithEndings::from(line) {
                let regions = highlighter
                    .highlight_line(l, &self.syntax_set)
                    .unwrap_or_default();
                for (style, s) in regions {
                    if s.is_empty() {
                        continue;
                    }
                    spans.push(Span::styled(s.to_string(), syn_style_to_ratatui(style)));
                }
            }
            if spans.is_empty() {
                spans.push(Span::raw((*line).to_string()));
            }
            out.push(spans);
        }
        out
    }
}

fn syn_style_to_ratatui(s: SynStyle) -> Style {
    let mut out = Style::default().fg(Color::Rgb(s.foreground.r, s.foreground.g, s.foreground.b));

    if s.font_style.contains(FontStyle::BOLD) {
        out = out.add_modifier(Modifier::BOLD);
    }
    if s.font_style.contains(FontStyle::ITALIC) {
        out = out.add_modifier(Modifier::ITALIC);
    }
    if s.font_style.contains(FontStyle::UNDERLINE) {
        out = out.add_modifier(Modifier::UNDERLINED);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_plain_text_without_panicking() {
        let h = SyntectHighlighter::new();
        let spans = h.highlight_line(Some("rs"), "fn main() {}\n");
        assert!(!spans.is_empty());

        let many = h.highlight_lines(Some("rs"), &["fn main() {", "}", ""]);
        assert_eq!(many.len(), 3);
    }
}
