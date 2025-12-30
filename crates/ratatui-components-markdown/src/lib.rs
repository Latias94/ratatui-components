//! Markdown rendering and interactive views for `ratatui-components`.
//!
//! This crate focuses on Glow-inspired terminal markdown rendering (headings, lists, tables,
//! blockquotes, code blocks, links, footnotes, etc.).
//!
//! ## Two layers
//!
//! - [`view::MarkdownView`]: interactive widget (viewport, scrolling, selection/copy).
//! - [`view::document`]: render core for custom layouts (parse once, render to `Text`).
//!
//! If you want to embed markdown into your own layout system (multi-pane, custom scrolling,
//! virtualization), prefer the render core in [`view::document`].
pub mod render;

#[cfg(feature = "mdstream")]
pub mod streaming;

pub mod view;

pub use view::document;
