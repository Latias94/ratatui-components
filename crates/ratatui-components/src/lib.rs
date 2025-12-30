//! High-level, batteries-included components for building modern TUIs with `ratatui`.
//!
//! This crate is a **facade**:
//! - It re-exports `ratatui-components-core` (always on).
//! - It conditionally exposes heavier components via feature flags (`markdown`, `syntect`,
//!   `treesitter`, `ansi`, `diff`, `transcript`, ...).
//!
//! ## Design notes
//!
//! - Event-loop agnostic (no runtime): you call `handle_event*` and `render*` from your app loop.
//! - Selection/copy is app-controlled: widgets emit `CopyRequested(String)` and do not touch the
//!   system clipboard by default.
//!
//! ## Recommended usage
//!
//! - If you only need core widgets (TextArea, DataGrid, VirtualList, CodeView), depend on
//!   `ratatui-components-core`.
//! - If you want markdown/diff/transcript/ANSI views, depend on this crate and enable features.
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
