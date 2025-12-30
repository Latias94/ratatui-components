use ratatui::style::Color;
use ratatui::text::Span;
use ratatui_components_core::text::CodeHighlighter;

#[cfg(feature = "syntect")]
use crate::syntect::SyntectHighlighter;
#[cfg(feature = "treesitter")]
use crate::treesitter::TreeSitterHighlighter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoHighlighterPreference {
    TreeSitter,
    Syntect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoHighlighterBackend {
    TreeSitter,
    Syntect,
    None,
}

pub struct AutoHighlighter {
    preference: AutoHighlighterPreference,
    background: Option<Color>,
    #[cfg(feature = "syntect")]
    syntect: SyntectHighlighter,
    #[cfg(feature = "treesitter")]
    treesitter: TreeSitterHighlighter,
}

impl Default for AutoHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl AutoHighlighter {
    pub fn new() -> Self {
        #[cfg(feature = "syntect")]
        let syntect = SyntectHighlighter::new();
        #[cfg(feature = "treesitter")]
        let mut treesitter = TreeSitterHighlighter::new();

        let background = {
            #[cfg(feature = "syntect")]
            {
                syntect.background_color()
            }
            #[cfg(all(not(feature = "syntect"), feature = "treesitter"))]
            {
                treesitter.background_color()
            }
            #[cfg(all(not(feature = "syntect"), not(feature = "treesitter")))]
            {
                None
            }
        };

        #[cfg(feature = "treesitter")]
        if let Some(bg) = background {
            treesitter.set_background(Some(bg));
        }

        Self {
            preference: AutoHighlighterPreference::TreeSitter,
            background,
            #[cfg(feature = "syntect")]
            syntect,
            #[cfg(feature = "treesitter")]
            treesitter,
        }
    }

    pub fn preference(&self) -> AutoHighlighterPreference {
        self.preference
    }

    pub fn set_preference(&mut self, preference: AutoHighlighterPreference) {
        self.preference = preference;
    }

    pub fn set_background_color(&mut self, background: Option<Color>) {
        self.background = background;
        #[cfg(feature = "treesitter")]
        {
            self.treesitter.set_background(background);
        }
    }

    pub fn backend_for_language(&self, language: Option<&str>) -> AutoHighlighterBackend {
        #[cfg(feature = "treesitter")]
        let treesitter_ok = language
            .and_then(|l| self.treesitter.supports_language(l).then_some(()))
            .is_some();
        #[cfg(not(feature = "treesitter"))]
        let treesitter_ok = false;

        #[cfg(feature = "syntect")]
        let syntect_ok = true;
        #[cfg(not(feature = "syntect"))]
        let syntect_ok = false;

        match self.preference {
            AutoHighlighterPreference::TreeSitter => {
                if treesitter_ok {
                    AutoHighlighterBackend::TreeSitter
                } else if syntect_ok {
                    AutoHighlighterBackend::Syntect
                } else {
                    AutoHighlighterBackend::None
                }
            }
            AutoHighlighterPreference::Syntect => {
                if syntect_ok {
                    AutoHighlighterBackend::Syntect
                } else if treesitter_ok {
                    AutoHighlighterBackend::TreeSitter
                } else {
                    AutoHighlighterBackend::None
                }
            }
        }
    }

    #[cfg(feature = "treesitter")]
    pub fn treesitter(&self) -> &TreeSitterHighlighter {
        &self.treesitter
    }

    #[cfg(feature = "treesitter")]
    pub fn treesitter_mut(&mut self) -> &mut TreeSitterHighlighter {
        &mut self.treesitter
    }

    #[cfg(feature = "syntect")]
    pub fn syntect(&self) -> &SyntectHighlighter {
        &self.syntect
    }

    #[cfg(feature = "syntect")]
    pub fn syntect_mut(&mut self) -> &mut SyntectHighlighter {
        &mut self.syntect
    }
}

impl CodeHighlighter for AutoHighlighter {
    fn background_color(&self) -> Option<Color> {
        self.background
    }

    fn highlight_lines(&self, language: Option<&str>, lines: &[&str]) -> Vec<Vec<Span<'static>>> {
        match self.backend_for_language(language) {
            #[cfg(feature = "treesitter")]
            AutoHighlighterBackend::TreeSitter => self.treesitter.highlight_lines(language, lines),
            #[cfg(feature = "syntect")]
            AutoHighlighterBackend::Syntect => self.syntect.highlight_lines(language, lines),
            AutoHighlighterBackend::None => lines
                .iter()
                .map(|l| vec![Span::raw((*l).to_string())])
                .collect(),
            #[allow(unreachable_patterns)]
            _ => lines
                .iter()
                .map(|l| vec![Span::raw((*l).to_string())])
                .collect(),
        }
    }
}

#[cfg(all(
    test,
    feature = "syntect",
    feature = "treesitter",
    feature = "treesitter-lang-rust"
))]
mod tests {
    use super::*;
    use ratatui::style::Style;

    #[test]
    fn prefers_treesitter_when_available() {
        let mut h = AutoHighlighter::new();
        h.set_preference(AutoHighlighterPreference::TreeSitter);
        h.treesitter_mut().theme_mut().keyword = Style::default().fg(Color::Indexed(123));
        h.treesitter_mut().refresh_styles();

        let spans = h.highlight_lines(Some("rs"), &["fn main() {}"]);
        let any_indexed_123 = spans
            .into_iter()
            .flatten()
            .any(|s| s.style.fg == Some(Color::Indexed(123)));
        assert!(any_indexed_123);
    }

    #[test]
    fn falls_back_to_syntect_when_treesitter_unavailable() {
        let h = AutoHighlighter::new();
        let spans = h.highlight_lines(Some("unknown-lang"), &["fn main() {}"]);
        assert_eq!(spans.len(), 1);
        assert!(!spans[0].is_empty());
    }
}
