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
    pub use tree_sitter_toml_ng as lang_toml;
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
            comment: Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
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
        } else if capture.contains("constant")
            || capture.contains("boolean")
            || capture.contains("builtin")
        {
            self.constant
        } else if capture.contains("property")
            || capture.contains("field")
            || capture.contains("attribute")
        {
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
        #[cfg(feature = "treesitter-lang-bash")]
        this.register_bash();
        #[cfg(feature = "treesitter-lang-python")]
        this.register_python();
        #[cfg(feature = "treesitter-lang-javascript")]
        this.register_javascript();
        #[cfg(feature = "treesitter-lang-typescript")]
        this.register_typescript();
        #[cfg(feature = "treesitter-lang-typescript")]
        this.register_tsx();
        #[cfg(feature = "treesitter-lang-json")]
        this.register_json();
        #[cfg(feature = "treesitter-lang-yaml")]
        this.register_yaml();
        #[cfg(feature = "treesitter-lang-toml")]
        this.register_toml();
        #[cfg(feature = "treesitter-lang-go")]
        this.register_go();
        #[cfg(feature = "treesitter-lang-html")]
        this.register_html();
        #[cfg(feature = "treesitter-lang-css")]
        this.register_css();
        #[cfg(feature = "treesitter-lang-c")]
        this.register_c();
        #[cfg(feature = "treesitter-lang-cpp")]
        this.register_cpp();
        #[cfg(feature = "treesitter-lang-java")]
        this.register_java();

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
        #[cfg(feature = "treesitter-lang-bash")]
        this.register_bash();
        #[cfg(feature = "treesitter-lang-python")]
        this.register_python();
        #[cfg(feature = "treesitter-lang-javascript")]
        this.register_javascript();
        #[cfg(feature = "treesitter-lang-typescript")]
        this.register_typescript();
        #[cfg(feature = "treesitter-lang-typescript")]
        this.register_tsx();
        #[cfg(feature = "treesitter-lang-json")]
        this.register_json();
        #[cfg(feature = "treesitter-lang-yaml")]
        this.register_yaml();
        #[cfg(feature = "treesitter-lang-toml")]
        this.register_toml();
        #[cfg(feature = "treesitter-lang-go")]
        this.register_go();
        #[cfg(feature = "treesitter-lang-html")]
        this.register_html();
        #[cfg(feature = "treesitter-lang-css")]
        this.register_css();
        #[cfg(feature = "treesitter-lang-c")]
        this.register_c();
        #[cfg(feature = "treesitter-lang-cpp")]
        this.register_cpp();
        #[cfg(feature = "treesitter-lang-java")]
        this.register_java();

        this
    }

    pub fn theme(&self) -> &TreeSitterTheme {
        &self.theme
    }

    pub fn theme_mut(&mut self) -> &mut TreeSitterTheme {
        &mut self.theme
    }

    pub fn set_theme(&mut self, theme: TreeSitterTheme) {
        self.theme = theme;
        self.refresh_styles();
    }

    pub fn set_background(&mut self, background: Option<Color>) {
        self.theme.background = background;
        self.refresh_styles();
    }

    pub fn refresh_styles(&mut self) {
        for entry in &mut self.languages {
            entry.styles = entry
                .config
                .names()
                .iter()
                .map(|name| self.theme.style_for_capture(name))
                .collect();
        }
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

    #[cfg(feature = "treesitter-lang-bash")]
    pub fn register_bash(&mut self) {
        let _ = self.register(
            "bash",
            langs::lang_bash::LANGUAGE.into(),
            langs::lang_bash::HIGHLIGHT_QUERY,
            "",
            "",
            ["sh", "bash", "shell", "zsh"],
        );
    }

    #[cfg(feature = "treesitter-lang-python")]
    pub fn register_python(&mut self) {
        let _ = self.register(
            "python",
            langs::lang_python::LANGUAGE.into(),
            langs::lang_python::HIGHLIGHTS_QUERY,
            "",
            "",
            ["py", "python"],
        );
    }

    #[cfg(feature = "treesitter-lang-javascript")]
    pub fn register_javascript(&mut self) {
        let _ = self.register(
            "javascript",
            langs::lang_javascript::LANGUAGE.into(),
            langs::lang_javascript::HIGHLIGHT_QUERY,
            langs::lang_javascript::INJECTIONS_QUERY,
            langs::lang_javascript::LOCALS_QUERY,
            ["js", "javascript", "jsx"],
        );
    }

    #[cfg(feature = "treesitter-lang-typescript")]
    pub fn register_typescript(&mut self) {
        let _ = self.register(
            "typescript",
            langs::lang_typescript::LANGUAGE_TYPESCRIPT.into(),
            langs::lang_typescript::HIGHLIGHTS_QUERY,
            "",
            langs::lang_typescript::LOCALS_QUERY,
            ["ts", "typescript"],
        );
    }

    #[cfg(feature = "treesitter-lang-typescript")]
    pub fn register_tsx(&mut self) {
        let _ = self.register(
            "tsx",
            langs::lang_typescript::LANGUAGE_TSX.into(),
            langs::lang_typescript::HIGHLIGHTS_QUERY,
            "",
            langs::lang_typescript::LOCALS_QUERY,
            ["tsx"],
        );
    }

    #[cfg(feature = "treesitter-lang-json")]
    pub fn register_json(&mut self) {
        let _ = self.register(
            "json",
            langs::lang_json::LANGUAGE.into(),
            langs::lang_json::HIGHLIGHTS_QUERY,
            "",
            "",
            ["json"],
        );
    }

    #[cfg(feature = "treesitter-lang-yaml")]
    pub fn register_yaml(&mut self) {
        let _ = self.register(
            "yaml",
            langs::lang_yaml::LANGUAGE.into(),
            langs::lang_yaml::HIGHLIGHTS_QUERY,
            "",
            "",
            ["yaml", "yml"],
        );
    }

    #[cfg(feature = "treesitter-lang-toml")]
    pub fn register_toml(&mut self) {
        let _ = self.register(
            "toml",
            langs::lang_toml::LANGUAGE.into(),
            langs::lang_toml::HIGHLIGHTS_QUERY,
            "",
            "",
            ["toml"],
        );
    }

    #[cfg(feature = "treesitter-lang-go")]
    pub fn register_go(&mut self) {
        let _ = self.register(
            "go",
            langs::lang_go::LANGUAGE.into(),
            langs::lang_go::HIGHLIGHTS_QUERY,
            "",
            "",
            ["go"],
        );
    }

    #[cfg(feature = "treesitter-lang-html")]
    pub fn register_html(&mut self) {
        let _ = self.register(
            "html",
            langs::lang_html::LANGUAGE.into(),
            langs::lang_html::HIGHLIGHTS_QUERY,
            langs::lang_html::INJECTIONS_QUERY,
            "",
            ["html", "htm"],
        );
    }

    #[cfg(feature = "treesitter-lang-css")]
    pub fn register_css(&mut self) {
        let _ = self.register(
            "css",
            langs::lang_css::LANGUAGE.into(),
            langs::lang_css::HIGHLIGHTS_QUERY,
            "",
            "",
            ["css"],
        );
    }

    #[cfg(feature = "treesitter-lang-c")]
    pub fn register_c(&mut self) {
        let _ = self.register(
            "c",
            langs::lang_c::LANGUAGE.into(),
            langs::lang_c::HIGHLIGHT_QUERY,
            "",
            "",
            ["c", "h"],
        );
    }

    #[cfg(feature = "treesitter-lang-cpp")]
    pub fn register_cpp(&mut self) {
        let _ = self.register(
            "cpp",
            langs::lang_cpp::LANGUAGE.into(),
            langs::lang_cpp::HIGHLIGHT_QUERY,
            "",
            "",
            ["cpp", "cc", "cxx", "hpp", "hh", "hxx"],
        );
    }

    #[cfg(feature = "treesitter-lang-java")]
    pub fn register_java(&mut self) {
        let _ = self.register(
            "java",
            langs::lang_java::LANGUAGE.into(),
            langs::lang_java::HIGHLIGHTS_QUERY,
            "",
            "",
            ["java"],
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

    pub fn supports_language(&self, language: &str) -> bool {
        self.keys.contains_key(language)
    }
}

impl CodeHighlighter for TreeSitterHighlighter {
    fn background_color(&self) -> Option<Color> {
        self.theme.background
    }

    fn highlight_text(&self, language: Option<&str>, text: &str) -> Vec<Vec<Span<'static>>> {
        let raw_lines: Vec<&str> = text.split('\n').collect();
        let Some(entry) = self.entry_for(language) else {
            return raw_lines
                .iter()
                .map(|l| vec![Span::raw((*l).to_string())])
                .collect();
        };

        let mut highlighter = Highlighter::new();
        let highlight_iter =
            match highlighter.highlight(&entry.config, text.as_bytes(), None, |_| {
                None::<&HighlightConfiguration>
            }) {
                Ok(it) => it,
                Err(_) => {
                    return raw_lines
                        .iter()
                        .map(|l| vec![Span::raw((*l).to_string())])
                        .collect();
                }
            };

        let mut out: Vec<Vec<Span<'static>>> = Vec::new();
        out.push(Vec::new());
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
                    let style = stack
                        .last()
                        .and_then(|h| entry.styles.get(h.0 as usize).copied())
                        .unwrap_or_default();

                    let mut s = &text[start..end];
                    while let Some(pos) = s.find('\n') {
                        let before = &s[..pos];
                        if !before.is_empty() {
                            if let Some(line) = out.last_mut() {
                                line.push(Span::styled(before.to_string(), style));
                            }
                        }
                        out.push(Vec::new());
                        s = &s[pos + 1..];
                    }
                    if !s.is_empty() {
                        if let Some(line) = out.last_mut() {
                            line.push(Span::styled(s.to_string(), style));
                        }
                    }
                }
                Err(_) => {
                    return raw_lines
                        .iter()
                        .map(|l| vec![Span::raw((*l).to_string())])
                        .collect();
                }
            }
        }

        if out.len() < raw_lines.len() {
            out.resize_with(raw_lines.len(), Vec::new);
        } else if out.len() > raw_lines.len() {
            out.truncate(raw_lines.len());
        }

        for (i, spans) in out.iter_mut().enumerate() {
            if spans.is_empty() {
                spans.push(Span::raw(
                    raw_lines.get(i).copied().unwrap_or("").to_string(),
                ));
            }
        }

        out
    }

    fn highlight_lines(&self, language: Option<&str>, lines: &[&str]) -> Vec<Vec<Span<'static>>> {
        let mut text = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                text.push('\n');
            }
            text.push_str(line);
        }
        self.highlight_text(language, &text)
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
        let any_styled = out.iter().flatten().any(|s| s.style != Style::default());
        assert!(any_styled);
    }

    #[test]
    fn highlights_other_enabled_languages_without_panicking() {
        let h = TreeSitterHighlighter::new();

        let cases: &mut Vec<(&str, Vec<&str>)> = &mut Vec::new();

        #[cfg(feature = "treesitter-lang-bash")]
        cases.push(("sh", vec!["echo hello", "x=1"]));
        #[cfg(feature = "treesitter-lang-python")]
        cases.push(("py", vec!["def f(x):", "  return x + 1"]));
        #[cfg(feature = "treesitter-lang-javascript")]
        cases.push(("js", vec!["function f(x) {", "  return x + 1;", "}"]));
        #[cfg(feature = "treesitter-lang-typescript")]
        cases.push((
            "ts",
            vec!["function f(x: number): number {", "  return x + 1;", "}"],
        ));
        #[cfg(feature = "treesitter-lang-typescript")]
        cases.push((
            "tsx",
            vec!["const x = <div>Hello</div>;", "export default x;"],
        ));
        #[cfg(feature = "treesitter-lang-json")]
        cases.push(("json", vec!["{", "  \"x\": 1", "}"]));
        #[cfg(feature = "treesitter-lang-yaml")]
        cases.push(("yaml", vec!["x: 1", "y: true"]));
        #[cfg(feature = "treesitter-lang-toml")]
        cases.push(("toml", vec!["[package]", "name = \"x\""]));
        #[cfg(feature = "treesitter-lang-go")]
        cases.push(("go", vec!["package main", "func main() {}"]));
        #[cfg(feature = "treesitter-lang-html")]
        cases.push(("html", vec!["<div>Hello</div>"]));
        #[cfg(feature = "treesitter-lang-css")]
        cases.push(("css", vec![".a { color: red; }"]));
        #[cfg(feature = "treesitter-lang-c")]
        cases.push(("c", vec!["int main() {", "  return 0;", "}"]));
        #[cfg(feature = "treesitter-lang-cpp")]
        cases.push(("cpp", vec!["int main() {", "  return 0;", "}"]));
        #[cfg(feature = "treesitter-lang-java")]
        cases.push(("java", vec!["class A {", "  void f() {}", "}"]));

        for (lang, lines) in cases.drain(..) {
            let out = h.highlight_lines(Some(lang), &lines);
            assert_eq!(out.len(), lines.len(), "lang={lang}");
            assert!(out.iter().all(|l| !l.is_empty()), "lang={lang}");
        }
    }
}
