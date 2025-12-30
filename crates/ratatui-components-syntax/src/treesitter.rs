use ratatui::style::Style;
use ratatui::text::Span;
use ratatui_components_core::text::CodeHighlighter;

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

pub struct TreeSitterHighlighter;

impl CodeHighlighter for TreeSitterHighlighter {
    fn highlight_lines(&self, _language: Option<&str>, lines: &[&str]) -> Vec<Vec<Span<'static>>> {
        lines
            .iter()
            .map(|l| vec![Span::styled((*l).to_string(), Style::default())])
            .collect()
    }
}
