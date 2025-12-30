//! Syntax highlighting abstractions and backends for `ratatui-components`.
//!
//! This crate provides a `CodeHighlighter`-style API (via `ratatui-components-core`) and optional
//! backends:
//! - `syntect` (feature: `syntect`)
//! - Tree-sitter (feature: `treesitter` + per-language features)
//!
//! The facade crate `ratatui-components` re-exports this crate behind feature flags, so most apps
//! can just enable `ratatui-components/syntect` or `ratatui-components/treesitter`.
#[cfg(feature = "syntect")]
pub mod syntect;

#[cfg(feature = "treesitter")]
pub mod treesitter;

pub mod auto;
