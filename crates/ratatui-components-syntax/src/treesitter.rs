use std::collections::HashMap;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui_components_core::text::CodeHighlighter;
use tree_sitter::Language;
use tree_sitter_highlight::{Highlight, HighlightConfiguration, Highlighter};

pub mod langs {
    #[cfg(feature = "treesitter-lang-bash")]
    pub use tree_sitter_bash as lang_bash;
    #[cfg(feature = "treesitter-lang-c")]
    pub use tree_sitter_c as lang_c;
    #[cfg(feature = "treesitter-lang-cpp")]
    pub use tree_sitter_cpp as lang_cpp;
    #[cfg(feature = "treesitter-lang-css")]
    pub use tree_sitter_css as lang_css;
    #[cfg(feature = "treesitter-lang-go")]
    pub use tree_sitter_go as lang_go;
    #[cfg(feature = "treesitter-lang-html")]
    pub use tree_sitter_html as lang_html;
    #[cfg(feature = "treesitter-lang-java")]
    pub use tree_sitter_java as lang_java;
    #[cfg(feature = "treesitter-lang-javascript")]
    pub use tree_sitter_javascript as lang_javascript;
    #[cfg(feature = "treesitter-lang-json")]
    pub use tree_sitter_json as lang_json;
    #[cfg(feature = "treesitter-lang-python")]
    pub use tree_sitter_python as lang_python;
    #[cfg(feature = "treesitter-lang-rust")]
    pub use tree_sitter_rust as lang_rust;
    #[cfg(feature = "treesitter-lang-toml")]
    pub use tree_sitter_toml as lang_toml;
    #[cfg(feature = "treesitter-lang-typescript")]
    pub use tree_sitter_typescript as lang_typescript;
    #[cfg(feature = "treesitter-lang-yaml")]
    pub use tree_sitter_yaml as lang_yaml;
}

#[derive(Clone, Debug)]
pub struct TreeSitterTheme {
    pub background: Option<Color>,
    pub keyword: Style,
    pub r#type: Style,
    pub function: Style,
    pub string: Style,
    pub number: Style,
    pub constant: Style,
    pub comment: Style,
    pub property: Style,
    pub variable: Style,
    pub punctuation: Style,
    pub operator: Style,
}

impl Default for TreeSitterTheme {
    fn default() -> Self {
        Self {
            background: None,
            keyword: Style::default().fg(Color::Magenta),
            r#type: Style::default().fg(Color::Yellow),
            function: Style::default().fg(Color::Cyan),
            string: Style::default().fg(Color::Green),
            number: Style::default().fg(Color::LightBlue),
            constant: Style::default().fg(Color::LightCyan),
            comment: Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            property: Style::default().fg(Color::LightMagenta),
            variable: Style::default().fg(Color::White),
            punctuation: Style::default().fg(Color::DarkGray),
            operator: Style::default().fg(Color::LightRed),
        }
    }
}

impl TreeSitterTheme {
    fn style_for_capture(&self, capture: &str) -> Style {
        let capture = capture.trim_start_matches('_');
        let mut style = if capture.contains("comment") {
            self.comment
        } else if capture.contains("string") {
            self.string
        } else if capture.contains("number") {
            self.number
        } else if capture.contains("keyword") {
            self.keyword
        } else if capture.contains("type") {
            self.r#type
        } else if capture.contains("function") || capture.contains("method") {
            self.function
        } else if capture.contains("constant") || capture.contains("boolean") || capture.contains("builtin") {
            self.constant
        } else if capture.contains("property") || capture.contains("field") || capture.contains("attribute") {
            self.property
        } else if capture.contains("variable") || capture.contains("parameter") {
            self.variable
        } else if capture.contains("operator") {
            self.operator
        } else if capture.contains("punctuation") {
            self.punctuation
        } else {
            Style::default()
        };

        if let Some(bg) = self.background {
            style = style.bg(bg);
        }
        style
    }
}

struct LanguageEntry {
    config: HighlightConfiguration,
    styles: Vec<Style>,
}

pub struct TreeSitterHighlighter {
    theme: TreeSitterTheme,
    languages: Vec<LanguageEntry>,
    keys: HashMap<&'static str, usize>,
}

impl Default for TreeSitterHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeSitterHighlighter {
    pub fn new() -> Self {
        let mut this = Self {
            theme: TreeSitterTheme::default(),
            languages: Vec::new(),
            keys: HashMap::new(),
        };

        #[cfg(feature = "treesitter-lang-rust")]
        this.register_rust();

        this
    }

    pub fn with_theme(theme: TreeSitterTheme) -> Self {
        let mut this = Self {
            theme,
            languages: Vec::new(),
            keys: HashMap::new(),
        };

        #[cfg(feature = "treesitter-lang-rust")]
        this.register_rust();

        this
    }

    pub fn theme(&self) -> &TreeSitterTheme {
        &self.theme
    }

    pub fn theme_mut(&mut self) -> &mut TreeSitterTheme {
        &mut self.theme
    }

    pub fn register(
        &mut self,
        language_name: impl Into<String>,
        language: Language,
        highlights_query: &'static str,
        injections_query: &'static str,
        locals_query: &'static str,
        keys: impl IntoIterator<Item = &'static str>,
    ) -> Result<(), tree_sitter::QueryError> {
        let mut config = HighlightConfiguration::new(
            language,
            language_name,
            highlights_query,
            injections_query,
            locals_query,
        )?;

        let capture_names: Vec<String> = config.names().iter().map(|s| (*s).to_string()).collect();
        config.configure(&capture_names);

        let styles = config
            .names()
            .iter()
            .map(|name| self.theme.style_for_capture(name))
            .collect();

        let idx = self.languages.len();
        self.languages.push(LanguageEntry { config, styles });
        for k in keys {
            self.keys.insert(k, idx);
        }
        Ok(())
    }

    #[cfg(feature = "treesitter-lang-rust")]
    pub fn register_rust(&mut self) {
        let _ = self.register(
            "rust",
            langs::lang_rust::LANGUAGE.into(),
            langs::lang_rust::HIGHLIGHTS_QUERY,
            langs::lang_rust::INJECTIONS_QUERY,
            "",
            ["rs", "rust"],
        );
    }

    fn entry_for(&self, language: Option<&str>) -> Option<&LanguageEntry> {
        let Some(lang) = language else {
            return None;
        };
        let Some(&idx) = self.keys.get(lang) else {
            return None;
        };
        self.languages.get(idx)
    }
}

impl CodeHighlighter for TreeSitterHighlighter {
    fn background_color(&self) -> Option<Color> {
        self.theme.background
    }

    fn highlight_lines(&self, language: Option<&str>, lines: &[&str]) -> Vec<Vec<Span<'static>>> {
        let Some(entry) = self.entry_for(language) else {
            return lines
                .iter()
                .map(|l| vec![Span::raw((*l).to_string())])
                .collect();
        };

        let source = lines.join("\n");
        let mut highlighter = Highlighter::new();
        let highlight_iter = match highlighter.highlight(&entry.config, source.as_bytes(), None, |_| {
            None::<&HighlightConfiguration>
        }) {
            Ok(it) => it,
            Err(_) => {
                return lines
                    .iter()
                    .map(|l| vec![Span::raw((*l).to_string())])
                    .collect();
            }
        };

        let mut out: Vec<Vec<Span<'static>>> = vec![Vec::new(); lines.len()];
        let mut line_idx = 0usize;
        let mut stack: Vec<Highlight> = Vec::new();

        for event in highlight_iter {
            match event {
                Ok(tree_sitter_highlight::HighlightEvent::HighlightStart(h)) => {
                    stack.push(h);
                }
                Ok(tree_sitter_highlight::HighlightEvent::HighlightEnd) => {
                    let _ = stack.pop();
                }
                Ok(tree_sitter_highlight::HighlightEvent::Source { start, end }) => {
                    if line_idx >= out.len() {
                        break;
                    }
                    let style = stack
                        .last()
                        .and_then(|h| entry.styles.get(h.0 as usize).copied())
                        .unwrap_or_default();

                    let mut s = &source[start..end];
                    while let Some(pos) = s.find('\n') {
                        let before = &s[..pos];
                        if !before.is_empty() && line_idx < out.len() {
                            out[line_idx].push(Span::styled(before.to_string(), style));
                        }
                        line_idx = line_idx.saturating_add(1);
                        if line_idx >= out.len() {
                            break;
                        }
                        s = &s[pos + 1..];
                    }
                    if line_idx < out.len() && !s.is_empty() {
                        out[line_idx].push(Span::styled(s.to_string(), style));
                    }
                }
                Err(_) => {
                    return lines
                        .iter()
                        .map(|l| vec![Span::raw((*l).to_string())])
                        .collect();
                }
            }
        }

        for (i, spans) in out.iter_mut().enumerate() {
            if spans.is_empty() {
                spans.push(Span::raw(lines.get(i).copied().unwrap_or("").to_string()));
            }
        }

        out
    }
}

#[cfg(all(test, feature = "treesitter-lang-rust"))]
mod tests {
    use super::*;

    #[test]
    fn highlights_rust_without_panicking() {
        let h = TreeSitterHighlighter::new();
        let out = h.highlight_lines(Some("rs"), &["fn main() {", "  let x = 1;", "}"]);
        assert_eq!(out.len(), 3);
        assert!(out.iter().all(|l| !l.is_empty()));
    }

    #[test]
    fn produces_some_non_default_styles() {
        let h = TreeSitterHighlighter::new();
        let out = h.highlight_lines(Some("rs"), &["fn main() {", "  let x = 1;", "}"]);
        let any_styled = out
            .iter()
            .flatten()
            .any(|s| s.style != Style::default());
        assert!(any_styled);
    }
}
