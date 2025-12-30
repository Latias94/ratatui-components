pub use ratatui_components_core::*;

#[cfg(feature = "ansi")]
pub mod ansi;
#[cfg(feature = "diff")]
pub mod diff;
#[cfg(feature = "transcript")]
pub mod transcript;

#[cfg(feature = "markdown")]
pub use ratatui_components_markdown as markdown;

#[cfg(any(feature = "syntect", feature = "treesitter"))]
pub use ratatui_components_syntax as syntax;

#[cfg(feature = "ansi")]
pub use ansi::AnsiTextView;
#[cfg(feature = "ansi")]
pub use ansi::AnsiTextViewOptions;
#[cfg(feature = "ansi")]
pub use ansi::ansi_text;

#[cfg(feature = "diff")]
pub use diff::DiffView;
#[cfg(feature = "diff")]
pub use diff::DiffViewOptions;

#[cfg(all(feature = "mdstream", feature = "markdown"))]
pub use markdown::streaming::MarkdownStreamView;
#[cfg(feature = "markdown")]
pub use markdown::view::LinkDestinationStyle;
#[cfg(feature = "markdown")]
pub use markdown::view::MarkdownView;
#[cfg(feature = "markdown")]
pub use markdown::view::MarkdownViewOptions;
#[cfg(feature = "markdown")]
pub use markdown::view::TableStyle;

#[cfg(feature = "transcript")]
pub use transcript::view::Role;
#[cfg(feature = "transcript")]
pub use transcript::view::TranscriptEntry;
#[cfg(feature = "transcript")]
pub use transcript::view::TranscriptView;
#[cfg(feature = "transcript")]
pub use transcript::view::TranscriptViewOptions;
