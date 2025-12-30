//! `ratatui-components-core` provides lightweight, reusable building blocks for terminal UIs.
//!
//! This crate is designed for **widget library authors** and apps that want fine-grained control.
//! Heavier functionality (Markdown parsing, syntax highlighting backends, ANSI parsing) lives in
//! separate feature-gated crates.
//!
//! ## Design goals
//!
//! - Event-loop agnostic: you drive input + rendering from your app.
//! - No async runtime: all components run on the main thread.
//! - Selection/copy is app-controlled: widgets emit [`selection::SelectionAction::CopyRequested`]
//!   and the caller decides how to integrate with a clipboard.
//!
//! ## Getting started
//!
//! Most users should depend on the facade crate `ratatui-components`. Use this crate directly if
//! you only need the core widgets/primitives.
//!
//! Useful entry points:
//! - [`code_view::CodeView`]: scrollable code viewer with selection + copy-on-request.
//! - [`textarea::TextArea`]: multi-line input with common editing semantics.
//! - [`virtual_list::VirtualListView`]: large list virtualization with keyboard navigation.
//! - [`datagrid::view::DataGridView`]: virtualized 2D grid.
//! - [`code_render::render_code_lines`]: render core for custom layouts.
//!
//! ## Selection / copy
//!
//! Widgets that support selection typically expose:
//! - a `handle_event_action*` method returning [`selection::SelectionAction`]
//! - a `selected_text()` method returning `Option<String>`
//!
//! Your app can map `CopyRequested(text)` into a clipboard action (or just show the copied text in
//! the UI).
pub mod theme;

pub mod text;

#[cfg(feature = "crossterm")]
pub mod crossterm_input;

pub mod render;
pub mod scroll;
pub mod selection;
pub mod viewport;
pub mod wrapping;

pub mod code_render;

pub mod code_view;
pub mod datagrid;
pub mod help;
pub mod input;
pub mod keymap;
pub mod textarea;
pub mod virtual_list;
