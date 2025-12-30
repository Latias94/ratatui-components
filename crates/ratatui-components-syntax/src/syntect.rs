use std::borrow::Cow;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui_components_core::text::CodeHighlighter;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Style as SynStyle, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
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

    pub fn with_theme(theme: Theme) -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        Self { syntax_set, theme }
    }

    pub fn theme_background_color(&self) -> Option<Color> {
        self.theme
            .settings
            .background
            .and_then(syntect_color_to_ratatui)
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
            'subline: for l in LinesWithEndings::from(line) {
                let had_newline = l.ends_with('\n');
                let l: Cow<'_, str> = if had_newline {
                    Cow::Borrowed(l)
                } else {
                    Cow::Owned(format!("{l}\n"))
                };

                let regions = match highlighter.highlight_line(l.as_ref(), &self.syntax_set) {
                    Ok(regions) => regions,
                    Err(_) => {
                        spans.clear();
                        spans.push(Span::raw((*line).to_string()));
                        break 'subline;
                    }
                };

                for (style, mut s) in regions {
                    if s.is_empty() {
                        continue;
                    }
                    if !had_newline && s.ends_with('\n') {
                        s = &s[..s.len() - 1];
                        if s.is_empty() {
                            continue;
                        }
                    }
                    spans.push(Span::styled(
                        s.to_string(),
                        syntect_style_to_ratatui(style),
                    ));
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

fn syntect_style_to_ratatui(s: SynStyle) -> Style {
    let mut out = Style::default();

    if let Some(fg) = syntect_color_to_ratatui(s.foreground) {
        out = out.fg(fg);
    }
    if let Some(bg) = syntect_color_to_ratatui(s.background) {
        out = out.bg(bg);
    }

    if s.font_style.intersects(FontStyle::BOLD) {
        out = out.add_modifier(Modifier::BOLD);
    }
    if s.font_style.intersects(FontStyle::ITALIC) {
        out = out.add_modifier(Modifier::ITALIC);
    }
    if s.font_style.intersects(FontStyle::UNDERLINE) {
        out = out.add_modifier(Modifier::UNDERLINED);
    }

    out
}

fn syntect_color_to_ratatui(color: syntect::highlighting::Color) -> Option<Color> {
    if color.a == 0 {
        Some(match color.r {
            0x00 => Color::Black,
            0x01 => Color::Red,
            0x02 => Color::Green,
            0x03 => Color::Yellow,
            0x04 => Color::Blue,
            0x05 => Color::Magenta,
            0x06 => Color::Cyan,
            0x07 => Color::Gray,
            0x08 => Color::DarkGray,
            0x09 => Color::LightRed,
            0x0A => Color::LightGreen,
            0x0B => Color::LightYellow,
            0x0C => Color::LightBlue,
            0x0D => Color::LightMagenta,
            0x0E => Color::LightCyan,
            0x0F => Color::White,
            c => Color::Indexed(c),
        })
    } else if color.a == 1 {
        None
    } else {
        Some(Color::Rgb(color.r, color.g, color.b))
    }
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

    #[test]
    fn converts_syntect_special_color_encoding() {
        use syntect::highlighting::Color as SynColor;

        assert_eq!(
            syntect_color_to_ratatui(SynColor {
                r: 0x00,
                g: 0,
                b: 0,
                a: 0
            }),
            Some(Color::Black)
        );
        assert_eq!(
            syntect_color_to_ratatui(SynColor {
                r: 0x10,
                g: 0,
                b: 0,
                a: 0
            }),
            Some(Color::Indexed(0x10))
        );
        assert_eq!(
            syntect_color_to_ratatui(SynColor {
                r: 1,
                g: 2,
                b: 3,
                a: 1
            }),
            None
        );
        assert_eq!(
            syntect_color_to_ratatui(SynColor {
                r: 1,
                g: 2,
                b: 3,
                a: 255
            }),
            Some(Color::Rgb(1, 2, 3))
        );
    }

    #[test]
    fn converts_syntect_style_background() {
        use syntect::highlighting::Color as SynColor;

        let s = SynStyle {
            foreground: SynColor {
                r: 10,
                g: 20,
                b: 30,
                a: 255,
            },
            background: SynColor {
                r: 0x00,
                g: 0,
                b: 0,
                a: 0,
            },
            font_style: FontStyle::BOLD | FontStyle::UNDERLINE,
        };

        let tui = syntect_style_to_ratatui(s);
        assert_eq!(tui.fg, Some(Color::Rgb(10, 20, 30)));
        assert_eq!(tui.bg, Some(Color::Black));
        assert!(tui.add_modifier.contains(Modifier::BOLD));
        assert!(tui.add_modifier.contains(Modifier::UNDERLINED));
    }
}
