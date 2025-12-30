use pulldown_cmark::Alignment;
use pulldown_cmark::CodeBlockKind;
use pulldown_cmark::CowStr;
use pulldown_cmark::Event;
use pulldown_cmark::HeadingLevel;
use pulldown_cmark::Options;
use pulldown_cmark::Parser;
use pulldown_cmark::Tag;
use pulldown_cmark::TagEnd;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use ratatui_components_core::input::InputEvent;
use ratatui_components_core::input::MouseButton;
use ratatui_components_core::input::MouseEvent;
use ratatui_components_core::input::MouseEventKind;
use ratatui_components_core::render;
use ratatui_components_core::scroll::ScrollBindings;
use ratatui_components_core::selection::SelectionAction;
use ratatui_components_core::selection::SelectionBindings;
use ratatui_components_core::text::CodeHighlighter;
use ratatui_components_core::theme::Theme;
use ratatui_components_core::viewport::ViewportState;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;
use url::Url;

#[derive(Clone, Copy, Debug, Default)]
struct InlineFlags {
    emphasis: bool,
    strong: bool,
    strike: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProseStyle {
    Normal,
    Heading(u8),
    BlockQuote,
    List,
}

#[derive(Clone, Debug)]
struct Segment {
    text: String,
    style: ProseStyle,
    flags: InlineFlags,
    inline_code: bool,
    link: bool,
    muted: bool,
}

impl Segment {
    fn new(text: String, style: ProseStyle, flags: InlineFlags) -> Self {
        Self {
            text,
            style,
            flags,
            inline_code: false,
            link: false,
            muted: false,
        }
    }
}

#[derive(Clone, Debug)]
struct ProseBlock {
    lines: Vec<Vec<Segment>>,
    initial_prefix: Vec<Segment>,
    subsequent_prefix: Vec<Segment>,
    wrap: bool,
}

#[derive(Clone, Debug)]
struct CodeBlock {
    language: Option<String>,
    lines: Arc<Vec<String>>,
    prefix: Vec<Segment>,
    indent: u16,
    highlight_key: u64,
}

#[derive(Clone, Debug)]
struct TableBlock {
    aligns: Vec<Alignment>,
    head: Vec<Vec<Vec<Segment>>>,
    body: Vec<Vec<Vec<Segment>>>,
    prefix: Vec<Segment>,
}

#[derive(Clone, Debug)]
enum Block {
    Prose(ProseBlock),
    Code(CodeBlock),
    Table(TableBlock),
    RuleIndented(Vec<Segment>),
    Blank(Vec<Segment>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableStyle {
    Glow,
    Box,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LinkDestinationStyle {
    /// `text (url)`
    #[default]
    Paren,
    /// `text url` (Glow-like)
    Space,
}

#[derive(Clone, Debug)]
pub struct MarkdownViewOptions {
    pub wrap_prose: bool,
    pub show_scrollbar: bool,
    pub preserve_new_lines: bool,
    pub show_link_destinations: bool,
    pub show_heading_markers: bool,
    pub glow_compat_relative_paths: bool,
    pub padding_left: u16,
    pub padding_right: u16,
    pub blockquote_prefix: String,
    pub table_style: TableStyle,
    pub glow_compat_quote_list_wrap: bool,
    pub glow_compat_loose_list_join: bool,
    pub glow_compat_post_list_blank_lines: u8,
    pub footnotes_at_end: bool,
    pub code_block_indent: u16,
    pub code_block_indent_in_blockquote: u16,
    pub footnote_hanging_indent: bool,
    pub show_code_line_numbers: bool,
    pub max_sync_highlight_lines: usize,
    pub max_sync_highlight_blocks_per_frame: usize,
    pub base_url: Option<String>,
    pub link_destination_style: LinkDestinationStyle,
    pub scroll: ScrollBindings,
    pub enable_selection: bool,
    pub selection: SelectionBindings,
}

impl Default for MarkdownViewOptions {
    fn default() -> Self {
        Self {
            wrap_prose: true,
            show_scrollbar: true,
            preserve_new_lines: false,
            show_link_destinations: false,
            show_heading_markers: false,
            glow_compat_relative_paths: false,
            padding_left: 0,
            padding_right: 0,
            blockquote_prefix: "| ".to_string(),
            table_style: TableStyle::Glow,
            glow_compat_quote_list_wrap: true,
            glow_compat_loose_list_join: false,
            glow_compat_post_list_blank_lines: 0,
            footnotes_at_end: false,
            code_block_indent: 4,
            code_block_indent_in_blockquote: 2,
            footnote_hanging_indent: true,
            show_code_line_numbers: false,
            max_sync_highlight_lines: 200,
            max_sync_highlight_blocks_per_frame: 1,
            base_url: None,
            link_destination_style: LinkDestinationStyle::Paren,
            scroll: ScrollBindings::default(),
            enable_selection: true,
            selection: SelectionBindings::default(),
        }
    }
}

pub mod document {
    //! A lightweight, reusable markdown rendering core.
    //!
    //! This module is intended for users who want to control layout themselves (multi-pane,
    //! custom scroll containers, virtualization, etc.) while reusing the same parsing and
    //! rendering logic as [`super::MarkdownView`].
    //!
    //! ## What you get
    //!
    //! - Parse markdown once into a [`MarkdownDocument`].
    //! - Render into a fully materialized [`ratatui::text::Text`] for a given `width` and
    //!   [`Theme`].
    //! - Optional syntax highlighting via a [`CodeHighlighter`].
    //!
    //! ## Minimal example
    //!
    //! ```rust,no_run
    //! use ratatui_components_core::theme::Theme;
    //! use ratatui_components_markdown::view::document::{MarkdownDocument, MarkdownRenderOptions};
    //!
    //! let options = MarkdownRenderOptions::default();
    //! let doc = MarkdownDocument::parse("# Hello\n\nSome *markdown*.", &options);
    //!
    //! // Cache this in your app state and only re-render on width/theme/source changes.
    //! let theme = Theme::default();
    //! let rendered = doc.render(80, &theme, &options, None);
    //! let text = rendered.into_text();
    //! # let _ = text;
    //! ```
    //!
    //! ## What you *don't* get (by design)
    //!
    //! This is a render core only. It intentionally does **not** include:
    //! - scrolling / viewport state
    //! - mouse/keyboard selection and hit-testing
    //! - clipboard integration
    //!
    //! If you want those, prefer [`super::MarkdownView`].
    //!
    //! ## Caching & layout best practices
    //!
    //! Rendering allocates a `Text<'static>` and can be non-trivial for large documents. In a
    //! typical TUI, you should:
    //! - keep a parsed [`MarkdownDocument`] in your app state
    //! - cache the latest [`RenderedMarkdown`] keyed by:
    //!   - `width` (layout changes)
    //!   - theme changes (if your theme is dynamic)
    //!   - highlighter changes (if you swap backends)
    //! - only re-render when one of those inputs changes
    //!
    //! For *very* large markdown content, note that this core currently renders the whole document
    //! into lines. You can still pair the result with a line-level virtualizer, but it will not
    //! avoid the upfront render cost.
    //!
    //! ## Selection / copy in custom layouts
    //!
    //! The core output is just `Text`. If you implement your own selection, a common pattern is to
    //! track an inclusive `(line, col)` → `(line, col)` range in terminal cell units and extract
    //! bytes using `ratatui_components_core::render::byte_range_for_cols_in_spans`.
    //!
    //! It intentionally does **not** expose lower-level internals (blocks/segments). APIs are
    //! expected to evolve until a 1.0 release.

    use super::*;

    /// Render-core configuration for [`MarkdownDocument`].
    ///
    /// Some options affect parsing (e.g. link destination policies, relative path resolution). For
    /// those, changing the option requires re-parsing via [`MarkdownDocument::parse`].
    #[derive(Clone, Debug)]
    pub struct MarkdownRenderOptions {
        pub wrap_prose: bool,
        pub preserve_new_lines: bool,
        pub show_link_destinations: bool,
        pub show_heading_markers: bool,
        pub glow_compat_relative_paths: bool,
        pub blockquote_prefix: String,
        pub table_style: TableStyle,
        pub glow_compat_quote_list_wrap: bool,
        pub glow_compat_loose_list_join: bool,
        pub glow_compat_post_list_blank_lines: u8,
        pub footnotes_at_end: bool,
        pub code_block_indent: u16,
        pub code_block_indent_in_blockquote: u16,
        pub footnote_hanging_indent: bool,
        pub show_code_line_numbers: bool,
        pub max_highlight_lines: usize,
        pub base_url: Option<String>,
        pub link_destination_style: LinkDestinationStyle,
    }

    impl Default for MarkdownRenderOptions {
        fn default() -> Self {
            Self {
                wrap_prose: true,
                preserve_new_lines: false,
                show_link_destinations: false,
                show_heading_markers: false,
                glow_compat_relative_paths: false,
                blockquote_prefix: "| ".to_string(),
                table_style: TableStyle::Glow,
                glow_compat_quote_list_wrap: true,
                glow_compat_loose_list_join: false,
                glow_compat_post_list_blank_lines: 0,
                footnotes_at_end: false,
                code_block_indent: 4,
                code_block_indent_in_blockquote: 2,
                footnote_hanging_indent: true,
                show_code_line_numbers: false,
                max_highlight_lines: 200,
                base_url: None,
                link_destination_style: LinkDestinationStyle::Paren,
            }
        }
    }

    impl From<&MarkdownViewOptions> for MarkdownRenderOptions {
        fn from(value: &MarkdownViewOptions) -> Self {
            Self {
                wrap_prose: value.wrap_prose,
                preserve_new_lines: value.preserve_new_lines,
                show_link_destinations: value.show_link_destinations,
                show_heading_markers: value.show_heading_markers,
                glow_compat_relative_paths: value.glow_compat_relative_paths,
                blockquote_prefix: value.blockquote_prefix.clone(),
                table_style: value.table_style,
                glow_compat_quote_list_wrap: value.glow_compat_quote_list_wrap,
                glow_compat_loose_list_join: value.glow_compat_loose_list_join,
                glow_compat_post_list_blank_lines: value.glow_compat_post_list_blank_lines,
                footnotes_at_end: value.footnotes_at_end,
                code_block_indent: value.code_block_indent,
                code_block_indent_in_blockquote: value.code_block_indent_in_blockquote,
                footnote_hanging_indent: value.footnote_hanging_indent,
                show_code_line_numbers: value.show_code_line_numbers,
                max_highlight_lines: value.max_sync_highlight_lines,
                base_url: value.base_url.clone(),
                link_destination_style: value.link_destination_style,
            }
        }
    }

    #[derive(Clone, Debug)]
    pub struct MarkdownDocument {
        source: String,
        blocks: Vec<Block>,
    }

    impl MarkdownDocument {
        /// Parses `source` into an opaque document representation.
        ///
        /// Note: options that affect parsing (links, footnotes, relative path resolution, etc.)
        /// are applied during `parse`. If you change those options, re-parse the document.
        pub fn parse(source: impl Into<String>, options: &MarkdownRenderOptions) -> Self {
            let source = source.into();
            let blocks = parse_markdown_blocks(
                &source,
                ParseOptions {
                    preserve_new_lines: options.preserve_new_lines,
                    show_link_destinations: options.show_link_destinations,
                    show_heading_markers: options.show_heading_markers,
                    glow_compat_relative_paths: options.glow_compat_relative_paths,
                    link_destination_style: options.link_destination_style,
                    glow_compat_quote_list_wrap: options.glow_compat_quote_list_wrap,
                    glow_compat_loose_list_join: options.glow_compat_loose_list_join,
                    glow_compat_post_list_blank_lines: options.glow_compat_post_list_blank_lines,
                    footnotes_at_end: options.footnotes_at_end,
                    blockquote_prefix: options.blockquote_prefix.as_str(),
                    code_block_indent: options.code_block_indent,
                    code_block_indent_in_blockquote: options.code_block_indent_in_blockquote,
                    footnote_hanging_indent: options.footnote_hanging_indent,
                    base_url: options.base_url.as_deref(),
                },
            );

            Self { source, blocks }
        }

        /// Returns the original markdown source (as provided to [`Self::parse`]).
        pub fn source(&self) -> &str {
            &self.source
        }

        /// Renders the document for a given terminal `width` and [`Theme`].
        ///
        /// This produces a fully materialized `Text<'static>` (owned lines/spans). It is designed
        /// to be cached by callers and reused across frames.
        ///
        /// ## Highlighting
        ///
        /// If `highlighter` is provided, code blocks are highlighted synchronously during this
        /// call. To keep worst-case frame times bounded, blocks longer than
        /// `options.max_highlight_lines` are skipped.
        ///
        /// Tip: pass an `Option<Arc<_>>` and clone it per render call; `Arc` cloning is cheap.
        pub fn render(
            &self,
            width: u16,
            theme: &Theme,
            options: &MarkdownRenderOptions,
            highlighter: Option<Arc<dyn CodeHighlighter + Send + Sync>>,
        ) -> RenderedMarkdown {
            if width == 0 {
                return RenderedMarkdown {
                    text: Text::default(),
                    content_width: 0,
                    content_height: 0,
                };
            }

            let mut rendered = layout_blocks(
                &self.blocks,
                width,
                options.wrap_prose,
                theme,
                options.show_code_line_numbers,
                options.table_style,
            );

            if let Some(hi) = highlighter {
                let mut highlight_cache: HashMap<u64, Arc<Vec<Vec<Span<'static>>>>> =
                    HashMap::new();
                for block in &self.blocks {
                    let Block::Code(code) = block else {
                        continue;
                    };
                    if highlight_cache.contains_key(&code.highlight_key) {
                        continue;
                    }
                    if code.lines.len() > options.max_highlight_lines {
                        continue;
                    }

                    let mut text = String::new();
                    for (i, line) in code.lines.iter().enumerate() {
                        if i > 0 {
                            text.push('\n');
                        }
                        text.push_str(line);
                    }
                    let highlighted = hi.highlight_text(code.language.as_deref(), &text);
                    highlight_cache.insert(code.highlight_key, Arc::new(highlighted));
                }

                for line in &mut rendered {
                    let Some(code_ref) = line.code_ref else {
                        continue;
                    };
                    let key = code_ref.highlight_key;
                    let Some(highlighted) = highlight_cache.get(&key) else {
                        line.code_ref = None;
                        continue;
                    };
                    let mut spans = line.spans[..code_ref.content_start].to_vec();
                    let content = highlighted
                        .get(code_ref.line_idx)
                        .cloned()
                        .unwrap_or_default();
                    if content.is_empty() {
                        spans.extend(line.spans[code_ref.content_start..].iter().cloned());
                    } else {
                        spans.extend(patch_spans_style(content, theme.code_inline));
                    }
                    line.spans = spans;
                    line.code_ref = None;
                }
            } else {
                for line in &mut rendered {
                    line.code_ref = None;
                }
            }

            let content_height = rendered.len() as u32;
            let content_width = rendered
                .iter()
                .map(|l| UnicodeWidthStr::width(l.plain.as_str()) as u32)
                .max()
                .unwrap_or(0);

            let text = Text::from(
                rendered
                    .into_iter()
                    .map(|l| Line::from(l.spans))
                    .collect::<Vec<_>>(),
            );

            RenderedMarkdown {
                text,
                content_width,
                content_height,
            }
        }
    }

    #[derive(Clone, Debug)]
    pub struct RenderedMarkdown {
        text: Text<'static>,
        content_width: u32,
        content_height: u32,
    }

    impl RenderedMarkdown {
        /// Returns the rendered [`Text`] without transferring ownership.
        pub fn text(&self) -> &Text<'static> {
            &self.text
        }

        /// Returns the rendered [`Text`], transferring ownership.
        pub fn into_text(self) -> Text<'static> {
            self.text
        }

        /// Returns `(content_width, content_height)` in terminal cell units.
        pub fn content_size(&self) -> (u32, u32) {
            (self.content_width, self.content_height)
        }
    }
}

/// An interactive Markdown viewer with scrolling, optional code highlighting, and selection/copy.
///
/// This view maintains internal caches keyed by `width` to avoid re-wrapping every frame.
///
/// For custom layouts (multi-pane, custom scroll containers, virtualization), use the render core
/// in [`document`].
#[derive(Default)]
pub struct MarkdownView {
    source: String,
    blocks: Vec<Block>,
    rendered: Vec<RenderedLine>,
    cached_width: Option<u16>,
    pub state: ViewportState,
    options: MarkdownViewOptions,
    highlighter: Option<Arc<dyn CodeHighlighter + Send + Sync>>,
    highlight_cache: HashMap<u64, Arc<Vec<Vec<Span<'static>>>>>,
    code_block_index: HashMap<u64, usize>,
    selection_anchor: Option<(usize, u32)>,
    selection: Option<((usize, u32), (usize, u32))>,
}

#[derive(Clone, Debug)]
struct RenderedLine {
    spans: Vec<Span<'static>>,
    plain: String,
    code_ref: Option<CodeRef>,
}

#[derive(Clone, Copy, Debug)]
struct CodeRef {
    highlight_key: u64,
    line_idx: usize,
    content_start: usize,
}

impl Clone for MarkdownView {
    fn clone(&self) -> Self {
        Self {
            source: self.source.clone(),
            blocks: self.blocks.clone(),
            rendered: self.rendered.clone(),
            cached_width: self.cached_width,
            state: self.state,
            options: self.options.clone(),
            highlighter: self.highlighter.clone(),
            highlight_cache: self.highlight_cache.clone(),
            code_block_index: self.code_block_index.clone(),
            selection_anchor: None,
            selection: None,
        }
    }
}

impl MarkdownView {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_options(options: MarkdownViewOptions) -> Self {
        Self {
            options,
            ..Default::default()
        }
    }

    /// Sets markdown source and reparses blocks.
    pub fn set_markdown(&mut self, input: &str) {
        self.source = input.to_string();
        self.blocks = parse_markdown_blocks(
            input,
            ParseOptions {
                preserve_new_lines: self.options.preserve_new_lines,
                show_link_destinations: self.options.show_link_destinations,
                show_heading_markers: self.options.show_heading_markers,
                glow_compat_relative_paths: self.options.glow_compat_relative_paths,
                link_destination_style: self.options.link_destination_style,
                glow_compat_quote_list_wrap: self.options.glow_compat_quote_list_wrap,
                glow_compat_loose_list_join: self.options.glow_compat_loose_list_join,
                glow_compat_post_list_blank_lines: self.options.glow_compat_post_list_blank_lines,
                footnotes_at_end: self.options.footnotes_at_end,
                blockquote_prefix: self.options.blockquote_prefix.as_str(),
                code_block_indent: self.options.code_block_indent,
                code_block_indent_in_blockquote: self.options.code_block_indent_in_blockquote,
                footnote_hanging_indent: self.options.footnote_hanging_indent,
                base_url: self.options.base_url.as_deref(),
            },
        );
        self.code_block_index.clear();
        for (idx, block) in self.blocks.iter().enumerate() {
            if let Block::Code(code) = block {
                self.code_block_index.insert(code.highlight_key, idx);
            }
        }
        self.cached_width = None;
        self.rendered.clear();
    }

    /// Sets an optional highlighter used for code blocks.
    pub fn set_highlighter(&mut self, highlighter: Option<Arc<dyn CodeHighlighter + Send + Sync>>) {
        self.highlighter = highlighter;
        self.cached_width = None;
        self.highlight_cache.clear();
    }

    /// Updates viewport size for `area` (and accounts for optional scrollbar/padding).
    pub fn set_viewport(&mut self, area: Rect) {
        let content_area = if self.options.show_scrollbar && area.width >= 2 {
            Rect::new(area.x, area.y, area.width - 1, area.height)
        } else {
            area
        };
        let content_area = inset_h(
            content_area,
            self.options.padding_left,
            self.options.padding_right,
        );
        self.state
            .set_viewport(content_area.width, content_area.height);
    }

    /// Scrolls vertically (y axis).
    pub fn scroll_y_by(&mut self, delta: i32) {
        self.state.scroll_y_by(delta);
    }

    /// Scrolls horizontally (x axis).
    pub fn scroll_x_by(&mut self, delta: i32) {
        self.state.scroll_x_by(delta);
    }

    /// Handles an event and returns a [`SelectionAction`] (redraw / copy-on-request).
    pub fn handle_event_action(&mut self, event: InputEvent) -> SelectionAction {
        match event {
            InputEvent::Paste(_) => SelectionAction::None,
            InputEvent::Mouse(m) => match m.kind {
                MouseEventKind::ScrollUp => {
                    self.state.scroll_y_by(-3);
                    SelectionAction::Redraw
                }
                MouseEventKind::ScrollDown => {
                    self.state.scroll_y_by(3);
                    SelectionAction::Redraw
                }
                _ => SelectionAction::None,
            },
            InputEvent::Key(key) => {
                if self.options.enable_selection && self.options.selection.is_clear(&key) {
                    self.clear_selection();
                    return SelectionAction::Redraw;
                }
                if self.options.enable_selection && self.options.selection.is_copy(&key) {
                    return self
                        .selected_text()
                        .map(SelectionAction::CopyRequested)
                        .unwrap_or(SelectionAction::None);
                }
                let Some(action) = self.options.scroll.action_for(&key) else {
                    return SelectionAction::None;
                };
                self.options.scroll.apply(&mut self.state, action);
                SelectionAction::Redraw
            }
        }
    }

    /// Like [`Self::handle_event_action`], but first updates viewport state for `area`.
    pub fn handle_event_action_in_area(
        &mut self,
        area: Rect,
        event: InputEvent,
    ) -> SelectionAction {
        match event {
            InputEvent::Paste(_) => SelectionAction::None,
            InputEvent::Key(_) => self.handle_event_action(event),
            InputEvent::Mouse(m) => {
                if self.handle_mouse_event(area, m) {
                    SelectionAction::Redraw
                } else {
                    SelectionAction::None
                }
            }
        }
    }

    /// Handles mouse events for scrolling and drag-selection.
    pub fn handle_mouse_event(&mut self, area: Rect, event: MouseEvent) -> bool {
        if area.width == 0 || area.height == 0 {
            return false;
        }

        self.set_viewport(area);

        match event.kind {
            MouseEventKind::ScrollUp => {
                self.state.scroll_y_by(-3);
                return true;
            }
            MouseEventKind::ScrollDown => {
                self.state.scroll_y_by(3);
                return true;
            }
            _ => {}
        }

        if !self.options.enable_selection {
            return false;
        }

        let (content_area, _) = if self.options.show_scrollbar && area.width >= 2 {
            (
                Rect::new(area.x, area.y, area.width - 1, area.height),
                Some(area.x + area.width - 1),
            )
        } else {
            (area, None)
        };

        let inner = inset_h(
            content_area,
            self.options.padding_left,
            self.options.padding_right,
        );
        self.ensure_layout(inner.width, &Theme::default());

        let content_start_x = inner.x;
        let content_end_x = inner.x.saturating_add(inner.width).saturating_sub(1);
        let content_start_y = content_area.y;
        let content_end_y = content_area
            .y
            .saturating_add(content_area.height)
            .saturating_sub(1);

        if content_start_x > content_end_x || content_start_y > content_end_y {
            return false;
        }

        let inside = event.x >= content_start_x
            && event.x <= content_end_x
            && event.y >= content_start_y
            && event.y <= content_end_y;

        let (x, y) = match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if !inside {
                    return false;
                }
                (event.x, event.y)
            }
            MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left) => {
                if self.selection_anchor.is_none() {
                    return false;
                }
                (
                    event.x.clamp(content_start_x, content_end_x),
                    event.y.clamp(content_start_y, content_end_y),
                )
            }
            _ => {
                if !inside {
                    return false;
                }
                (event.x, event.y)
            }
        };

        let rel_x = (x - inner.x) as u32;
        let rel_y = (y - content_area.y) as u32;
        let line = self
            .state
            .y
            .saturating_add(rel_y)
            .min(self.rendered.len().saturating_sub(1) as u32) as usize;
        let col = self.state.x.saturating_add(rel_x);

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.selection_anchor = Some((line, col));
                self.selection = Some(((line, col), (line, col)));
                true
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                let Some(anchor) = self.selection_anchor else {
                    return false;
                };
                self.selection = Some((anchor, (line, col)));
                true
            }
            MouseEventKind::Up(MouseButton::Left) => {
                let Some(anchor) = self.selection_anchor else {
                    return false;
                };
                self.selection = Some((anchor, (line, col)));
                self.selection_anchor = None;
                true
            }
            _ => false,
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
        self.selection = None;
    }

    pub fn selected_text(&mut self) -> Option<String> {
        let ((l0, c0), (l1, c1)) = self.selection?;
        let ((start_line, start_col), (end_line, end_col)) =
            normalize_sel_inclusive((l0, c0), (l1, c1));

        let theme = Theme::default();
        self.ensure_layout(self.cached_width.unwrap_or(80), &theme);
        self.materialize_highlights(0, self.rendered.len(), &theme);

        let mut out = String::new();
        for line_idx in start_line..=end_line {
            let line = self.rendered.get(line_idx)?;
            if line_idx > start_line {
                out.push('\n');
            }

            let (from, to) = if start_line == end_line {
                (start_col, end_col)
            } else if line_idx == start_line {
                (start_col, u32::MAX)
            } else if line_idx == end_line {
                (0, end_col)
            } else {
                (0, u32::MAX)
            };

            if let Some((bs, be)) = render::byte_range_for_cols_in_spans(&line.spans, from, to) {
                out.push_str(&render::slice_spans_by_bytes(&line.spans, bs, be));
            }
        }
        Some(out)
    }

    pub fn render_ref(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let (content_area, scrollbar_x) = if self.options.show_scrollbar && area.width >= 2 {
            (
                Rect::new(area.x, area.y, area.width - 1, area.height),
                Some(area.x + area.width - 1),
            )
        } else {
            (area, None)
        };

        self.set_viewport(area);
        let inner = inset_h(
            content_area,
            self.options.padding_left,
            self.options.padding_right,
        );
        self.ensure_layout(inner.width, theme);
        let start = self.state.y as usize;
        let end = (start + content_area.height as usize).min(self.rendered.len());
        self.materialize_highlights(start, end, theme);

        for row in 0..content_area.height {
            let y = content_area.y + row;
            let idx = (self.state.y as usize).saturating_add(row as usize);
            let line = self.rendered.get(idx);
            buf.set_style(
                Rect::new(content_area.x, y, content_area.width, 1),
                theme.text_primary,
            );
            if let Some(line) = line {
                if self.options.enable_selection
                    && let Some(((l0, c0), (l1, c1))) = self.selection
                {
                    let ((start_line, start_col), (end_line, end_col)) =
                        normalize_sel_inclusive((l0, c0), (l1, c1));
                    if idx >= start_line && idx <= end_line {
                        let (from, to) = if start_line == end_line {
                            (start_col, end_col)
                        } else if idx == start_line {
                            (start_col, u32::MAX)
                        } else if idx == end_line {
                            (0, end_col)
                        } else {
                            (0, u32::MAX)
                        };
                        if let Some((bs, be)) =
                            render::byte_range_for_cols_in_spans(&line.spans, from, to)
                        {
                            let spans = render::apply_modifier_to_byte_ranges(
                                line.spans.clone(),
                                &[(bs, be)],
                                Modifier::REVERSED,
                            );
                            render::render_spans_clipped(
                                inner.x,
                                y,
                                self.state.x,
                                inner.width,
                                buf,
                                &spans,
                                theme.text_primary,
                            );
                            continue;
                        }
                    }
                }

                render::render_spans_clipped(
                    inner.x,
                    y,
                    self.state.x,
                    inner.width,
                    buf,
                    &line.spans,
                    theme.text_primary,
                );
            }
        }

        if let Some(sb_x) = scrollbar_x {
            render::render_scrollbar(
                Rect::new(sb_x, area.y, 1, area.height),
                buf,
                &self.state,
                theme.text_muted,
            );
        }
    }

    pub fn lines_for_width(&mut self, width: u16, theme: &Theme) -> Vec<Line<'static>> {
        let width = width
            .saturating_sub(self.options.padding_left)
            .saturating_sub(self.options.padding_right);
        self.ensure_layout(width, theme);
        self.ensure_all_highlights();
        self.materialize_highlights(0, self.rendered.len(), theme);
        self.rendered
            .iter()
            .map(|l| Line::from(l.spans.clone()))
            .collect()
    }

    pub fn as_text(&mut self) -> Text<'static> {
        let theme = Theme::default();
        self.ensure_layout(self.cached_width.unwrap_or(80), &theme);
        self.ensure_all_highlights();
        self.materialize_highlights(0, self.rendered.len(), &theme);
        Text::from(
            self.rendered
                .iter()
                .cloned()
                .map(|l| Line::from(l.spans))
                .collect::<Vec<_>>(),
        )
    }

    fn ensure_layout(&mut self, width: u16, theme: &Theme) {
        if self.cached_width == Some(width) && !self.rendered.is_empty() {
            return;
        }
        self.cached_width = Some(width);
        self.rendered = layout_blocks(
            &self.blocks,
            width,
            self.options.wrap_prose,
            theme,
            self.options.show_code_line_numbers,
            self.options.table_style,
        );
        let content_h = self.rendered.len() as u32;
        let content_w = self
            .rendered
            .iter()
            .map(|l| UnicodeWidthStr::width(l.plain.as_str()) as u32)
            .max()
            .unwrap_or(0);
        self.state.set_content(content_w, content_h);
    }

    fn ensure_all_highlights(&mut self) {
        let Some(hi) = self.highlighter.clone() else {
            return;
        };

        for block in &self.blocks {
            let Block::Code(code) = block else {
                continue;
            };
            if self.highlight_cache.contains_key(&code.highlight_key) {
                continue;
            }
            let mut text = String::new();
            for (i, line) in code.lines.iter().enumerate() {
                if i > 0 {
                    text.push('\n');
                }
                text.push_str(line);
            }
            let highlighted = hi.highlight_text(code.language.as_deref(), &text);
            self.highlight_cache
                .insert(code.highlight_key, Arc::new(highlighted));
        }
    }

    fn materialize_highlights(&mut self, start: usize, end: usize, theme: &Theme) {
        let Some(hi) = self.highlighter.clone() else {
            return;
        };
        let end = end.min(self.rendered.len());
        if start >= end {
            return;
        }

        let mut sync_budget = self.options.max_sync_highlight_blocks_per_frame;

        for idx in start..end {
            let Some(code_ref) = self.rendered[idx].code_ref else {
                continue;
            };

            let key = code_ref.highlight_key;
            let highlighted = self.highlight_cache.get(&key).cloned().or_else(|| {
                if sync_budget == 0 {
                    return None;
                }
                let block_idx = *self.code_block_index.get(&key)?;
                let Block::Code(code) = self.blocks.get(block_idx)? else {
                    return None;
                };
                if code.lines.len() > self.options.max_sync_highlight_lines {
                    return None;
                }
                let mut text = String::new();
                for (i, line) in code.lines.iter().enumerate() {
                    if i > 0 {
                        text.push('\n');
                    }
                    text.push_str(line);
                }
                let highlighted = hi.highlight_text(code.language.as_deref(), &text);
                let highlighted = Arc::new(highlighted);
                self.highlight_cache.insert(key, highlighted.clone());
                sync_budget = sync_budget.saturating_sub(1);
                Some(highlighted)
            });

            let highlighted = match highlighted {
                Some(v) => v,
                None => continue,
            };

            let mut spans = self.rendered[idx].spans[..code_ref.content_start].to_vec();
            let content = highlighted
                .get(code_ref.line_idx)
                .cloned()
                .unwrap_or_default();

            if content.is_empty() {
                spans.extend(
                    self.rendered[idx].spans[code_ref.content_start..]
                        .iter()
                        .cloned(),
                );
            } else {
                spans.extend(patch_spans_style(content, theme.code_inline));
            }

            self.rendered[idx].spans = spans;
            self.rendered[idx].code_ref = None;
        }
    }
}

fn normalize_sel(a: (usize, u32), b: (usize, u32)) -> ((usize, u32), (usize, u32)) {
    if a.0 < b.0 || (a.0 == b.0 && a.1 <= b.1) {
        (a, b)
    } else {
        (b, a)
    }
}

fn normalize_sel_inclusive(a: (usize, u32), b: (usize, u32)) -> ((usize, u32), (usize, u32)) {
    let (start, end) = normalize_sel(a, b);
    (start, (end.0, end.1.saturating_add(1)))
}

fn inset_h(area: Rect, left: u16, right: u16) -> Rect {
    let left = left.min(area.width);
    let right = right.min(area.width.saturating_sub(left));
    Rect::new(
        area.x.saturating_add(left),
        area.y,
        area.width.saturating_sub(left).saturating_sub(right),
        area.height,
    )
}

#[derive(Clone, Copy, Debug)]
struct ParseOptions<'a> {
    preserve_new_lines: bool,
    show_link_destinations: bool,
    show_heading_markers: bool,
    glow_compat_relative_paths: bool,
    link_destination_style: LinkDestinationStyle,
    glow_compat_quote_list_wrap: bool,
    glow_compat_loose_list_join: bool,
    glow_compat_post_list_blank_lines: u8,
    footnotes_at_end: bool,
    blockquote_prefix: &'a str,
    code_block_indent: u16,
    code_block_indent_in_blockquote: u16,
    footnote_hanging_indent: bool,
    base_url: Option<&'a str>,
}

fn parse_markdown_blocks(input: &str, opts: ParseOptions<'_>) -> Vec<Block> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(input, options);

    #[derive(Clone, Debug)]
    struct IndentCtx {
        initial: Vec<Segment>,
        subsequent: Vec<Segment>,
        use_subsequent_for_initial: bool,
    }

    #[derive(Clone, Debug)]
    struct ListCtx {
        ordered: bool,
        index: u64,
    }

    #[derive(Clone, Debug)]
    struct ItemCtx {
        has_block: bool,
        indent_idx: usize,
    }

    struct Builder<'a> {
        blocks: Vec<Block>,
        footnote_blocks: Vec<Block>,
        indent_stack: Vec<IndentCtx>,
        list_stack: Vec<ListCtx>,
        item_stack: Vec<ItemCtx>,
        blockquote_depth: usize,
        last_was_footnote_definition: bool,
        pending_loose_list_join: bool,
        pending_post_list_blank_lines: u8,
        in_paragraph: bool,
        current_style: ProseStyle,
        inline: InlineFlags,
        in_link: bool,
        link_text: String,
        para_lines: Vec<Vec<Segment>>,
        para_current: Vec<Segment>,
        para_prefix_initial: Vec<Segment>,
        para_prefix_subsequent: Vec<Segment>,
        para_wrap: bool,
        in_code_block: bool,
        code_language: Option<String>,
        code_lines: Vec<String>,
        code_current: String,
        code_prefix: Vec<Segment>,
        code_indent: u16,
        code_block_indent: u16,
        code_block_indent_in_blockquote: u16,
        wants_blank: bool,
        preserve_new_lines: bool,

        in_table: bool,
        table_aligns: Vec<Alignment>,
        in_table_head: bool,
        table_row: Vec<Vec<Segment>>,
        table_cell: Vec<Segment>,
        table_head: Vec<Vec<Vec<Segment>>>,
        table_body: Vec<Vec<Vec<Segment>>>,
        in_table_cell: bool,
        table_prefix: Vec<Segment>,

        link_dest: Option<String>,
        image_dest: Option<String>,
        image_alt: String,
        show_link_destinations: bool,
        show_heading_markers: bool,
        glow_compat_relative_paths: bool,
        link_destination_style: LinkDestinationStyle,
        glow_compat_quote_list_wrap: bool,
        glow_compat_loose_list_join: bool,
        glow_compat_post_list_blank_lines: u8,
        footnotes_at_end: bool,
        blockquote_prefix: String,
        footnote_hanging_indent: bool,
        in_footnote_definition: bool,
        saved_stacks: Option<SavedStacks>,
        base_url: Option<&'a str>,
    }

    type SavedStacks = (Vec<IndentCtx>, Vec<ListCtx>, Vec<ItemCtx>, bool);

    impl<'a> Builder<'a> {
        fn new(opts: ParseOptions<'a>) -> Self {
            Self {
                blocks: Vec::new(),
                footnote_blocks: Vec::new(),
                indent_stack: Vec::new(),
                list_stack: Vec::new(),
                item_stack: Vec::new(),
                blockquote_depth: 0,
                last_was_footnote_definition: false,
                pending_loose_list_join: false,
                pending_post_list_blank_lines: 0,
                in_paragraph: false,
                current_style: ProseStyle::Normal,
                inline: InlineFlags::default(),
                in_link: false,
                link_text: String::new(),
                para_lines: Vec::new(),
                para_current: Vec::new(),
                para_prefix_initial: Vec::new(),
                para_prefix_subsequent: Vec::new(),
                para_wrap: true,
                in_code_block: false,
                code_language: None,
                code_lines: Vec::new(),
                code_current: String::new(),
                code_prefix: Vec::new(),
                code_indent: opts.code_block_indent,
                code_block_indent: opts.code_block_indent,
                code_block_indent_in_blockquote: opts.code_block_indent_in_blockquote,
                wants_blank: false,
                preserve_new_lines: opts.preserve_new_lines,

                in_table: false,
                table_aligns: Vec::new(),
                in_table_head: false,
                table_row: Vec::new(),
                table_cell: Vec::new(),
                table_head: Vec::new(),
                table_body: Vec::new(),
                in_table_cell: false,
                table_prefix: Vec::new(),

                link_dest: None,
                image_dest: None,
                image_alt: String::new(),
                show_link_destinations: opts.show_link_destinations,
                show_heading_markers: opts.show_heading_markers,
                glow_compat_relative_paths: opts.glow_compat_relative_paths,
                link_destination_style: opts.link_destination_style,
                glow_compat_quote_list_wrap: opts.glow_compat_quote_list_wrap,
                glow_compat_loose_list_join: opts.glow_compat_loose_list_join,
                glow_compat_post_list_blank_lines: opts.glow_compat_post_list_blank_lines,
                footnotes_at_end: opts.footnotes_at_end,
                blockquote_prefix: opts.blockquote_prefix.to_string(),
                footnote_hanging_indent: opts.footnote_hanging_indent,
                in_footnote_definition: false,
                saved_stacks: None,
                base_url: opts.base_url,
            }
        }

        fn resolve_dest(&self, dest: &str) -> String {
            if self.base_url.is_none()
                && self.glow_compat_relative_paths
                && let Some(rest) = dest.strip_prefix("./")
            {
                return format!("/{rest}");
            }
            resolve_url(self.base_url, dest)
        }

        fn blocks_mut(&mut self) -> &mut Vec<Block> {
            if self.in_footnote_definition {
                &mut self.footnote_blocks
            } else {
                &mut self.blocks
            }
        }

        fn blocks(&self) -> &Vec<Block> {
            if self.in_footnote_definition {
                &self.footnote_blocks
            } else {
                &self.blocks
            }
        }

        fn maybe_blank(&mut self) {
            let should_push =
                self.wants_blank && !matches!(self.blocks().last(), None | Some(Block::Blank(_)));
            if should_push {
                let (_, subsequent) = self.snapshot_prefixes();
                self.blocks_mut().push(Block::Blank(subsequent));
            }
            self.wants_blank = false;
        }

        fn force_blank(&mut self) {
            if matches!(self.blocks().last(), None | Some(Block::Blank(_))) {
                return;
            }
            let (_, subsequent) = self.snapshot_prefixes();
            self.blocks_mut().push(Block::Blank(subsequent));
        }

        fn push_blank_line(&mut self) {
            let (_, subsequent) = self.snapshot_prefixes();
            self.blocks_mut().push(Block::Blank(subsequent));
        }

        fn mark_item_has_block(&mut self) {
            if let Some(item) = self.item_stack.last_mut() {
                item.has_block = true;
                if let Some(ctx) = self.indent_stack.get_mut(item.indent_idx) {
                    ctx.use_subsequent_for_initial = true;
                }
            }
        }

        fn snapshot_prefixes(&self) -> (Vec<Segment>, Vec<Segment>) {
            let mut initial: Vec<Segment> = Vec::new();
            let mut subsequent: Vec<Segment> = Vec::new();
            for ctx in &self.indent_stack {
                if ctx.use_subsequent_for_initial {
                    initial.extend(ctx.subsequent.clone());
                } else {
                    initial.extend(ctx.initial.clone());
                }
                subsequent.extend(ctx.subsequent.clone());
            }
            (initial, subsequent)
        }

        fn flush_para(&mut self) {
            if !self.in_paragraph {
                return;
            }
            if !self.para_current.is_empty() {
                self.para_lines.push(std::mem::take(&mut self.para_current));
            }
            let lines = std::mem::take(&mut self.para_lines);
            if !lines.is_empty() {
                self.maybe_blank();
                let initial_prefix = self.para_prefix_initial.clone();
                let subsequent_prefix = self.para_prefix_subsequent.clone();
                let wrap = self.para_wrap;
                let block = Block::Prose(ProseBlock {
                    lines,
                    initial_prefix,
                    subsequent_prefix,
                    wrap,
                });
                self.blocks_mut().push(block);
                self.mark_item_has_block();
                self.wants_blank = self.list_stack.is_empty() && !self.in_table;
            }
            self.in_paragraph = false;
            self.pending_loose_list_join = false;
        }

        fn flush_code(&mut self) {
            if !self.in_code_block {
                return;
            }
            if !self.code_current.is_empty() {
                self.code_lines.push(std::mem::take(&mut self.code_current));
            }
            if self.code_lines.last().is_some_and(|s| s.is_empty()) {
                self.code_lines.pop();
            }
            self.maybe_blank();
            let language = self.code_language.take();
            let lines = std::mem::take(&mut self.code_lines);
            let highlight_key = highlight_cache_key(language.as_deref(), &lines);
            let lines = Arc::new(lines);
            let prefix = std::mem::take(&mut self.code_prefix);
            let indent = self.code_indent;
            self.blocks_mut().push(Block::Code(CodeBlock {
                language,
                lines,
                prefix,
                indent,
                highlight_key,
            }));
            self.mark_item_has_block();
            self.in_code_block = false;
            self.wants_blank = self.list_stack.is_empty() && !self.in_table;
        }

        fn start_paragraph(&mut self, style: ProseStyle) {
            self.flush_code();
            self.flush_para();
            self.maybe_blank();
            self.in_paragraph = true;
            self.current_style = match style {
                ProseStyle::Normal if self.blockquote_depth > 0 => ProseStyle::BlockQuote,
                other => other,
            };
            let (i, mut s) = self.snapshot_prefixes();
            if self.glow_compat_quote_list_wrap
                && !self.item_stack.is_empty()
                && s.iter()
                    .any(|seg| matches!(seg.style, ProseStyle::BlockQuote) && !is_all_ws(&seg.text))
            {
                s.retain(|seg| {
                    !matches!(seg.style, ProseStyle::BlockQuote) || is_all_ws(&seg.text)
                });
            }
            self.para_prefix_initial = i;
            self.para_prefix_subsequent = s;
            self.para_wrap = true;
        }

        fn ensure_paragraph(&mut self) {
            if self.in_paragraph {
                return;
            }
            self.maybe_blank();
            self.in_paragraph = true;
            self.current_style = if self.blockquote_depth > 0 {
                ProseStyle::BlockQuote
            } else {
                ProseStyle::Normal
            };
            let (i, mut s) = self.snapshot_prefixes();
            if self.glow_compat_quote_list_wrap
                && !self.item_stack.is_empty()
                && s.iter()
                    .any(|seg| matches!(seg.style, ProseStyle::BlockQuote) && !is_all_ws(&seg.text))
            {
                s.retain(|seg| {
                    !matches!(seg.style, ProseStyle::BlockQuote) || is_all_ws(&seg.text)
                });
            }
            self.para_prefix_initial = i;
            self.para_prefix_subsequent = s;
            self.para_wrap = true;
        }
        fn flush_table(&mut self) {
            if !self.in_table {
                return;
            }
            self.flush_para();
            self.flush_code();
            self.maybe_blank();
            let aligns = std::mem::take(&mut self.table_aligns);
            let head = std::mem::take(&mut self.table_head);
            let body = std::mem::take(&mut self.table_body);
            let prefix = std::mem::take(&mut self.table_prefix);
            self.blocks_mut().push(Block::Table(TableBlock {
                aligns,
                head,
                body,
                prefix,
            }));
            self.mark_item_has_block();
            self.in_table = false;
            self.in_table_head = false;
            self.table_row.clear();
            self.table_cell.clear();
            self.wants_blank = self.list_stack.is_empty();
        }

        fn task_marker(&mut self, checked: bool) {
            let Some(ctx) = self.indent_stack.last_mut() else {
                return;
            };
            if ctx.initial.is_empty() || ctx.subsequent.is_empty() {
                return;
            }
            // Glow renders task list items as "[✓]" and "[ ]" (no bullet).
            let marker = if checked { "[✓] " } else { "[ ] " }.to_string();
            let marker_width = UnicodeWidthStr::width(marker.as_str());
            ctx.initial[0].text = marker;
            ctx.subsequent[0].text = " ".repeat(marker_width);
        }
    }

    let mut b = Builder::new(opts);

    fn push_inline(b: &mut Builder, seg: Segment) {
        if b.in_table && b.in_table_cell {
            b.table_cell.push(seg);
        } else {
            b.ensure_paragraph();
            b.para_current.push(seg);
        }
    }

    for ev in parser {
        match ev {
            Event::Start(tag) => {
                if !matches!(tag, Tag::Paragraph) {
                    b.pending_post_list_blank_lines = 0;
                }
                match tag {
                    Tag::Paragraph => {
                        if b.glow_compat_loose_list_join
                            && b.pending_loose_list_join
                            && !b.item_stack.is_empty()
                        {
                            b.pending_loose_list_join = false;
                            continue;
                        }
                        if b.pending_post_list_blank_lines > 0 && b.list_stack.is_empty() {
                            for _ in 0..b.pending_post_list_blank_lines {
                                b.push_blank_line();
                            }
                            b.pending_post_list_blank_lines = 0;
                        }
                        if b.item_stack.last().is_some_and(|item| item.has_block) {
                            b.force_blank();
                        }
                        b.start_paragraph(ProseStyle::Normal);
                    }
                    Tag::Heading { level, .. } => {
                        if b.item_stack.last().is_some_and(|item| item.has_block) {
                            b.force_blank();
                        }
                        let hl = heading_level(level);
                        b.start_paragraph(ProseStyle::Heading(hl));
                        if b.show_heading_markers {
                            let hashes = "#".repeat(level as usize);
                            let inline = b.inline;
                            push_inline(
                                &mut b,
                                Segment::new(format!("{hashes} "), ProseStyle::Heading(hl), inline),
                            );
                        }
                    }
                    Tag::BlockQuote(_) => {
                        b.flush_code();
                        b.flush_para();
                        if b.item_stack.last().is_some_and(|item| item.has_block) {
                            b.force_blank();
                        }
                        b.maybe_blank();
                        b.blockquote_depth += 1;
                        b.indent_stack.push(IndentCtx {
                            initial: vec![Segment::new(
                                b.blockquote_prefix.clone(),
                                ProseStyle::BlockQuote,
                                b.inline,
                            )],
                            subsequent: vec![Segment::new(
                                b.blockquote_prefix.clone(),
                                ProseStyle::BlockQuote,
                                b.inline,
                            )],
                            use_subsequent_for_initial: false,
                        });
                    }
                    Tag::FootnoteDefinition(label) => {
                        b.flush_table();
                        b.flush_code();
                        b.flush_para();

                        if b.footnotes_at_end {
                            b.in_footnote_definition = true;
                            b.saved_stacks = Some((
                                std::mem::take(&mut b.indent_stack),
                                std::mem::take(&mut b.list_stack),
                                std::mem::take(&mut b.item_stack),
                                b.wants_blank,
                            ));
                            b.wants_blank = b.footnote_blocks.is_empty();
                        }

                        if b.last_was_footnote_definition {
                            b.wants_blank = false;
                        }
                        b.maybe_blank();
                        let marker = format!("[^{label}]: ");
                        let marker_width = UnicodeWidthStr::width(marker.as_str());
                        b.indent_stack.push(IndentCtx {
                            initial: vec![Segment::new(marker, ProseStyle::List, b.inline)],
                            subsequent: if b.footnote_hanging_indent {
                                vec![Segment::new(
                                    " ".repeat(marker_width),
                                    ProseStyle::List,
                                    b.inline,
                                )]
                            } else {
                                vec![Segment::new(String::new(), ProseStyle::List, b.inline)]
                            },
                            use_subsequent_for_initial: false,
                        });
                        b.last_was_footnote_definition = false;
                    }
                    Tag::List(start) => {
                        let ordered = start.is_some();
                        let index = start.unwrap_or(1);
                        b.list_stack.push(ListCtx { ordered, index });
                    }
                    Tag::Item => {
                        b.flush_code();
                        b.flush_para();
                        let Some(list) = b.list_stack.last() else {
                            continue;
                        };
                        let indent_idx = b.indent_stack.len();
                        b.item_stack.push(ItemCtx {
                            has_block: false,
                            indent_idx,
                        });
                        let marker = if list.ordered {
                            format!("{}. ", list.index)
                        } else {
                            "• ".to_string()
                        };
                        let marker_width = UnicodeWidthStr::width(marker.as_str());
                        b.indent_stack.push(IndentCtx {
                            initial: vec![Segment::new(marker, ProseStyle::List, b.inline)],
                            subsequent: vec![Segment::new(
                                " ".repeat(marker_width),
                                ProseStyle::List,
                                b.inline,
                            )],
                            use_subsequent_for_initial: false,
                        });
                    }
                    Tag::Emphasis => b.inline.emphasis = true,
                    Tag::Strong => b.inline.strong = true,
                    Tag::Strikethrough => b.inline.strike = true,
                    Tag::Link { dest_url, .. } => {
                        b.in_link = true;
                        b.link_dest = Some(b.resolve_dest(dest_url.as_ref()));
                        b.link_text.clear();
                    }
                    Tag::Image { dest_url, .. } => {
                        b.image_dest = Some(b.resolve_dest(dest_url.as_ref()));
                        b.image_alt.clear();
                    }
                    Tag::CodeBlock(kind) => {
                        b.flush_para();
                        b.flush_code();
                        if b.item_stack.last().is_some_and(|item| item.has_block) {
                            b.force_blank();
                        }
                        b.maybe_blank();
                        b.in_code_block = true;
                        b.code_lines.clear();
                        b.code_current.clear();
                        let (_, subsequent) = b.snapshot_prefixes();
                        b.code_prefix = subsequent;
                        b.code_indent = if b.blockquote_depth > 0 {
                            b.code_block_indent_in_blockquote
                        } else {
                            b.code_block_indent
                        };
                        match kind {
                            CodeBlockKind::Fenced(lang) => {
                                b.code_language = normalize_fenced_lang(&lang);
                            }
                            CodeBlockKind::Indented => b.code_language = None,
                        }
                    }
                    Tag::Table(aligns) => {
                        b.flush_para();
                        b.flush_code();
                        if b.item_stack.last().is_some_and(|item| item.has_block) {
                            b.force_blank();
                        }
                        b.maybe_blank();
                        b.in_table = true;
                        let (_, subsequent) = b.snapshot_prefixes();
                        b.table_prefix = subsequent;
                        b.table_aligns = aligns;
                        b.table_head.clear();
                        b.table_body.clear();
                        b.table_row.clear();
                        b.table_cell.clear();
                        b.in_table_head = false;
                        b.in_table_cell = false;
                    }
                    Tag::TableHead => {
                        b.in_table_head = true;
                        b.table_row.clear();
                    }
                    Tag::TableRow => {
                        b.table_row.clear();
                    }
                    Tag::TableCell => {
                        b.in_table_cell = true;
                        b.table_cell.clear();
                    }
                    _ => {}
                }
            }
            Event::End(tag) => match tag {
                TagEnd::Paragraph => {
                    if b.glow_compat_loose_list_join && !b.item_stack.is_empty() {
                        b.pending_loose_list_join = true;
                    } else {
                        b.flush_para();
                    }
                }
                TagEnd::Heading(_) => b.flush_para(),
                TagEnd::BlockQuote(_) => {
                    b.flush_para();
                    b.indent_stack.pop();
                    b.blockquote_depth = b.blockquote_depth.saturating_sub(1);
                }
                TagEnd::FootnoteDefinition => {
                    b.flush_para();
                    b.indent_stack.pop();
                    b.last_was_footnote_definition = true;
                    if b.in_footnote_definition {
                        b.in_footnote_definition = false;
                        if let Some((indent, list, items, wants_blank)) = b.saved_stacks.take() {
                            b.indent_stack = indent;
                            b.list_stack = list;
                            b.item_stack = items;
                            b.wants_blank = wants_blank;
                        }
                    }
                }
                TagEnd::List(_) => {
                    b.flush_para();
                    b.list_stack.pop();
                    b.wants_blank = b.list_stack.is_empty() && !b.in_table;
                    if b.list_stack.is_empty() && b.glow_compat_post_list_blank_lines > 0 {
                        b.pending_post_list_blank_lines = b.glow_compat_post_list_blank_lines;
                    }
                }
                TagEnd::Item => {
                    b.flush_para();
                    if let Some(list) = b.list_stack.last_mut()
                        && list.ordered
                    {
                        list.index += 1;
                    }
                    b.item_stack.pop();
                    b.indent_stack.pop();
                }
                TagEnd::Emphasis => b.inline.emphasis = false,
                TagEnd::Strong => b.inline.strong = false,
                TagEnd::Strikethrough => b.inline.strike = false,
                TagEnd::Link => {
                    b.in_link = false;
                    if b.show_link_destinations
                        && let Some(url) = b.link_dest.as_deref()
                    {
                        let text = b.link_text.trim();
                        let show = !text.is_empty()
                            && text != url
                            && text != url.strip_prefix("mailto:").unwrap_or(url);
                        if show {
                            let suffix = match b.link_destination_style {
                                LinkDestinationStyle::Paren => format!(" ({url})"),
                                LinkDestinationStyle::Space => format!(" {url}"),
                            };
                            let mut seg =
                                Segment::new(suffix, ProseStyle::Normal, InlineFlags::default());
                            seg.muted = true;
                            push_inline(&mut b, seg);
                        }
                    }
                    b.link_dest = None;
                    b.link_text.clear();
                }
                TagEnd::Image => {
                    let alt = b.image_alt.trim().to_string();
                    let alt = if alt.is_empty() {
                        "[image]".to_string()
                    } else {
                        alt
                    };
                    let mut label = Segment::new(
                        "Image: ".to_string(),
                        ProseStyle::Normal,
                        InlineFlags::default(),
                    );
                    label.muted = true;
                    push_inline(&mut b, label);

                    let mut alt_seg = Segment::new(alt, ProseStyle::Normal, b.inline);
                    alt_seg.link = true;
                    push_inline(&mut b, alt_seg);

                    let mut arrow = Segment::new(
                        " → ".to_string(),
                        ProseStyle::Normal,
                        InlineFlags::default(),
                    );
                    arrow.muted = true;
                    push_inline(&mut b, arrow);

                    if let Some(url) = b.image_dest.take() {
                        let mut url_seg =
                            Segment::new(url, ProseStyle::Normal, InlineFlags::default());
                        url_seg.muted = true;
                        push_inline(&mut b, url_seg);
                    }
                }
                TagEnd::CodeBlock => b.flush_code(),
                TagEnd::TableCell => {
                    b.in_table_cell = false;
                    b.table_row.push(std::mem::take(&mut b.table_cell));
                }
                TagEnd::TableRow => {
                    if b.in_table_head {
                        b.table_head.push(std::mem::take(&mut b.table_row));
                    } else {
                        b.table_body.push(std::mem::take(&mut b.table_row));
                    }
                }
                TagEnd::TableHead => {
                    b.in_table_head = false;
                    if !b.table_row.is_empty() {
                        b.table_head.push(std::mem::take(&mut b.table_row));
                    }
                }
                TagEnd::Table => b.flush_table(),
                _ => {}
            },
            Event::Text(text) => {
                if b.in_code_block {
                    for ch in text.chars() {
                        match ch {
                            '\n' => b.code_lines.push(std::mem::take(&mut b.code_current)),
                            '\r' => {}
                            '\t' => b.code_current.push_str("    "),
                            other => b.code_current.push(other),
                        }
                    }
                    continue;
                }
                if b.image_dest.is_some() {
                    b.image_alt.push_str(text.as_ref());
                    continue;
                }
                if b.in_link {
                    b.link_text.push_str(text.as_ref());
                }
                if b.in_table && b.in_table_cell {
                    push_text_segments(
                        &mut b.table_cell,
                        &text,
                        b.current_style,
                        b.inline,
                        false,
                        b.in_link,
                    );
                } else {
                    b.ensure_paragraph();
                    push_text_segments(
                        &mut b.para_current,
                        &text,
                        b.current_style,
                        b.inline,
                        false,
                        b.in_link,
                    );
                }
            }
            Event::Code(code) => {
                if b.image_dest.is_some() {
                    b.image_alt.push_str(code.as_ref());
                    continue;
                }
                if b.in_link {
                    b.link_text.push_str(code.as_ref());
                }
                let mut seg = Segment::new(code.to_string(), b.current_style, b.inline);
                seg.inline_code = true;
                seg.link = b.in_link;
                push_inline(&mut b, seg);
            }
            Event::SoftBreak => {
                if b.in_code_block {
                    b.code_lines.push(std::mem::take(&mut b.code_current));
                    continue;
                }
                if b.in_table && b.in_table_cell {
                    b.table_cell
                        .push(Segment::new(" ".to_string(), b.current_style, b.inline));
                    continue;
                }
                if b.in_paragraph {
                    if b.preserve_new_lines {
                        b.para_lines.push(std::mem::take(&mut b.para_current));
                    } else {
                        b.para_current.push(Segment::new(
                            " ".to_string(),
                            b.current_style,
                            b.inline,
                        ));
                    }
                }
                if b.in_link {
                    b.link_text.push(' ');
                }
            }
            Event::HardBreak => {
                if b.in_code_block {
                    b.code_lines.push(std::mem::take(&mut b.code_current));
                    continue;
                }
                if b.in_table && b.in_table_cell {
                    b.table_cell
                        .push(Segment::new(" ".to_string(), b.current_style, b.inline));
                    continue;
                }
                if b.in_paragraph {
                    b.para_lines.push(std::mem::take(&mut b.para_current));
                }
                if b.in_link {
                    b.link_text.push(' ');
                }
            }
            Event::Rule => {
                b.flush_para();
                b.flush_code();
                if b.item_stack.last().is_some_and(|item| item.has_block) {
                    b.force_blank();
                }
                b.maybe_blank();
                let (_, subsequent) = b.snapshot_prefixes();
                b.blocks.push(Block::RuleIndented(subsequent));
                b.mark_item_has_block();
                b.wants_blank = b.list_stack.is_empty();
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                let text = html_to_text(&html);
                if !text.trim().is_empty() {
                    let mut seg = Segment::new(text, b.current_style, b.inline);
                    seg.muted = true;
                    push_inline(&mut b, seg);
                }
            }
            Event::InlineMath(math) => {
                let mut seg = Segment::new(format!("${math}$"), b.current_style, b.inline);
                seg.inline_code = true;
                push_inline(&mut b, seg);
            }
            Event::DisplayMath(math) => {
                b.start_paragraph(ProseStyle::Normal);
                let mut seg = Segment::new(format!("$$ {math} $$"), b.current_style, b.inline);
                seg.inline_code = true;
                push_inline(&mut b, seg);
                b.flush_para();
            }
            Event::FootnoteReference(label) => {
                let marker = format!("[^{label}]");
                if b.in_table && b.in_table_cell {
                    if let Some(last) = b.table_cell.last_mut()
                        && !last.text.ends_with(char::is_whitespace)
                        && !last.text.is_empty()
                    {
                        last.text.push_str(&marker);
                    } else {
                        let style = b.current_style;
                        let flags = b.inline;
                        let mut seg = Segment::new(marker, style, flags);
                        seg.link = true;
                        b.table_cell.push(seg);
                    }
                    continue;
                }

                b.ensure_paragraph();
                if let Some(last) = b.para_current.last_mut()
                    && !last.text.ends_with(char::is_whitespace)
                    && !last.text.is_empty()
                {
                    last.text.push_str(&marker);
                } else {
                    let style = b.current_style;
                    let flags = b.inline;
                    let mut seg = Segment::new(marker, style, flags);
                    seg.link = true;
                    b.para_current.push(seg);
                }
            }
            Event::TaskListMarker(checked) => {
                b.task_marker(checked);
            }
        }
    }

    b.flush_para();
    b.flush_code();
    b.flush_table();

    while matches!(b.blocks.last(), Some(Block::Blank(_))) {
        b.blocks.pop();
    }

    while matches!(b.footnote_blocks.last(), Some(Block::Blank(_))) {
        b.footnote_blocks.pop();
    }

    if b.footnotes_at_end && !b.footnote_blocks.is_empty() {
        if !b.blocks.is_empty() && !matches!(b.blocks.last(), Some(Block::Blank(_))) {
            b.blocks.push(Block::Blank(Vec::new()));
        }
        let first_non_blank = b
            .footnote_blocks
            .iter()
            .position(|b| !matches!(b, Block::Blank(_)))
            .unwrap_or(b.footnote_blocks.len());
        b.footnote_blocks.drain(0..first_non_blank);
        b.blocks.extend(b.footnote_blocks);
    }

    b.blocks
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn push_text_segments(
    out: &mut Vec<Segment>,
    text: &CowStr<'_>,
    style: ProseStyle,
    flags: InlineFlags,
    inline_code: bool,
    link: bool,
) {
    let mut s = Segment::new(text.to_string(), style, flags);
    s.inline_code = inline_code;
    s.link = link;
    out.push(s);
}

fn normalize_fenced_lang(lang: &CowStr<'_>) -> Option<String> {
    let raw = lang.trim();
    let first = raw.split_whitespace().next().unwrap_or("");
    let first = first.split(',').next().unwrap_or("").trim();
    if first.is_empty() {
        return None;
    }
    let first = first.strip_prefix("language-").unwrap_or(first);
    let first = first.strip_prefix('{').unwrap_or(first);
    let first = first.strip_suffix('}').unwrap_or(first);
    let first = first.trim();
    if first.is_empty() {
        None
    } else {
        Some(first.to_string())
    }
}

fn html_to_text(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if in_tag => {}
            _ => out.push(ch),
        }
    }
    normalize_whitespace(&decode_basic_html_entities(&out))
}

fn decode_basic_html_entities(s: &str) -> String {
    // Keep this minimal: it covers the most common entities produced by Markdown/HTML snippets.
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn normalize_whitespace(s: &str) -> String {
    let mut out = String::new();
    let mut last_was_ws = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !last_was_ws {
                out.push(' ');
                last_was_ws = true;
            }
        } else {
            out.push(ch);
            last_was_ws = false;
        }
    }
    out.trim().to_string()
}

fn resolve_url(base_url: Option<&str>, dest: &str) -> String {
    let dest = dest.trim();
    if dest.is_empty() {
        return String::new();
    }
    if is_absolute_url(dest) {
        return dest.to_string();
    }
    let Some(base) = base_url.map(str::trim).filter(|s| !s.is_empty()) else {
        return dest.to_string();
    };

    if let Ok(base) = Url::parse(base) {
        return base
            .join(dest)
            .map(|u| u.to_string())
            .unwrap_or_else(|_| dest.to_string());
    }

    let mut base = base
        .trim_end_matches('/')
        .trim_end_matches('\\')
        .to_string();
    let mut dest = dest.to_string();
    while dest.starts_with("./") {
        dest = dest[2..].to_string();
    }
    while dest.starts_with('/') {
        dest = dest[1..].to_string();
    }
    base.push('/');
    base.push_str(&dest);
    base
}

fn is_absolute_url(dest: &str) -> bool {
    let d = dest.trim();
    d.starts_with('#')
        || d.starts_with("mailto:")
        || d.starts_with("http://")
        || d.starts_with("https://")
        || d.starts_with("file://")
        || d.starts_with('/')
}

fn highlight_cache_key(language: Option<&str>, lines: &[String]) -> u64 {
    let mut h = DefaultHasher::new();
    language.unwrap_or("").hash(&mut h);
    for line in lines {
        line.hash(&mut h);
        '\n'.hash(&mut h);
    }
    h.finish()
}

fn digits(n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    let mut d = 0;
    let mut v = n;
    while v > 0 {
        v /= 10;
        d += 1;
    }
    d
}

fn layout_blocks(
    blocks: &[Block],
    width: u16,
    wrap_prose: bool,
    theme: &Theme,
    show_code_line_numbers: bool,
    table_style: TableStyle,
) -> Vec<RenderedLine> {
    let mut out: Vec<RenderedLine> = Vec::new();
    for b in blocks {
        match b {
            Block::Blank(prefix) => {
                let spans = if prefix.is_empty() {
                    vec![Span::raw("")]
                } else {
                    segments_to_spans(prefix, theme)
                };
                let plain = join_spans_plain(&spans);
                out.push(RenderedLine {
                    spans,
                    plain,
                    code_ref: None,
                });
            }
            Block::RuleIndented(prefix) => {
                let mut spans = segments_to_spans(prefix, theme);
                let prefix_cols =
                    UnicodeWidthStr::width(join_segments_plain(prefix).as_str()) as u16;
                let remaining = width.saturating_sub(prefix_cols);
                let rule_len = remaining.clamp(1, 8) as usize;
                spans.push(Span::styled("-".repeat(rule_len), theme.text_muted));
                let plain = join_spans_plain(&spans);
                out.push(RenderedLine {
                    spans,
                    plain,
                    code_ref: None,
                });
            }
            Block::Code(code) => {
                let prefix_spans = segments_to_spans(&code.prefix, theme);
                let indent_span = if code.indent > 0 {
                    Some(Span::styled(
                        " ".repeat(code.indent as usize),
                        theme.code_inline,
                    ))
                } else {
                    None
                };

                let show_line_numbers = show_code_line_numbers;
                let line_number_w = if show_line_numbers {
                    digits(code.lines.len())
                } else {
                    1
                };

                let mut content_start = prefix_spans.len() + usize::from(indent_span.is_some());
                if show_line_numbers {
                    // We insert two spans before the actual code content: "{n:>w} │ " and then the code content.
                    // `content_start` must point to the start of the code content span.
                    content_start += 1;
                }
                for (line_idx, line) in code.lines.iter().enumerate() {
                    let mut spans: Vec<Span<'static>> = prefix_spans.clone();
                    if let Some(indent) = indent_span.clone() {
                        spans.push(indent);
                    }
                    if show_line_numbers {
                        let n = line_idx + 1;
                        let gutter = format!("{n:>width$} │ ", width = line_number_w);
                        spans.push(Span::styled(
                            gutter,
                            theme.code_inline.patch(theme.text_muted),
                        ));
                    }
                    spans.push(Span::styled(line.clone(), theme.code_inline));
                    let plain = join_spans_plain(&spans);
                    out.push(RenderedLine {
                        spans,
                        plain,
                        code_ref: Some(CodeRef {
                            highlight_key: code.highlight_key,
                            line_idx,
                            content_start,
                        }),
                    });
                }
            }
            Block::Table(table) => {
                out.extend(layout_table(table, width, theme, table_style));
            }
            Block::Prose(p) => {
                let wrap = wrap_prose && p.wrap;
                for (i, logical) in p.lines.iter().enumerate() {
                    let initial_prefix = if i == 0 {
                        &p.initial_prefix
                    } else {
                        &p.subsequent_prefix
                    };
                    if wrap {
                        out.extend(wrap_segments(
                            initial_prefix,
                            &p.subsequent_prefix,
                            logical,
                            width,
                            theme,
                        ));
                    } else {
                        let mut spans = segments_to_spans(initial_prefix, theme);
                        spans.extend(segments_to_spans(logical, theme));
                        let plain = join_spans_plain(&spans);
                        out.push(RenderedLine {
                            spans,
                            plain,
                            code_ref: None,
                        });
                    }
                }
            }
        }
    }
    out
}

fn layout_table(
    table: &TableBlock,
    width: u16,
    theme: &Theme,
    style: TableStyle,
) -> Vec<RenderedLine> {
    match style {
        TableStyle::Glow => layout_table_glow(table, width, theme),
        TableStyle::Box => layout_table_box(table, width, theme),
    }
}

fn layout_table_glow(table: &TableBlock, width: u16, theme: &Theme) -> Vec<RenderedLine> {
    let prefix_spans = segments_to_spans(&table.prefix, theme);
    let prefix_cols = UnicodeWidthStr::width(join_segments_plain(&table.prefix).as_str()) as u16;
    let width = width.saturating_sub(prefix_cols);
    // Glow leaves a small horizontal gutter around tables; keep a tiny margin to match.
    let width = width.saturating_sub(2);

    let cols = table
        .head
        .iter()
        .chain(table.body.iter())
        .map(|row| row.len())
        .max()
        .unwrap_or(0);
    if cols == 0 || width == 0 {
        return Vec::new();
    }

    let (head_rows, body_rows) = if !table.head.is_empty() {
        (table.head.as_slice(), table.body.as_slice())
    } else if !table.body.is_empty() {
        (&table.body[0..1], &table.body[1..])
    } else {
        (&[][..], &[][..])
    };

    let sep_w = cols.saturating_sub(1) as u16; // '|'
    let padding_w = 2u16 * cols as u16; // 1 space on each side inside cell
    let chrome_w = sep_w.saturating_add(padding_w);
    if chrome_w >= width {
        return Vec::new();
    }

    let min_col_w = 1u16;
    let mut col_w: Vec<u16> = vec![min_col_w; cols];
    for row in table.head.iter().chain(table.body.iter()) {
        for (ci, cell) in row.iter().enumerate() {
            let w = UnicodeWidthStr::width(join_segments_plain(cell).as_str()) as u16;
            col_w[ci] = col_w[ci].max(w).max(1);
        }
    }

    let available = width.saturating_sub(chrome_w);
    let min_total = min_col_w.saturating_mul(cols as u16);
    if min_total > available {
        let per = (available / cols as u16).max(1);
        col_w.fill(per);
    } else {
        let mut total = col_w.iter().copied().sum::<u16>();
        while total > available {
            if let Some((idx, _)) = col_w
                .iter()
                .enumerate()
                .filter(|(_, w)| **w > min_col_w)
                .max_by_key(|(_, w)| **w)
            {
                col_w[idx] -= 1;
                total -= 1;
            } else {
                break;
            }
        }
    }

    let mut out: Vec<RenderedLine> = Vec::new();

    for row in head_rows {
        out.extend(layout_table_row_glow(
            row,
            &col_w,
            &table.aligns,
            theme,
            true,
        ));
    }
    if !head_rows.is_empty() {
        out.push(RenderedLine {
            spans: vec![Span::styled(
                table_separator_line_glow(&col_w),
                theme.text_muted,
            )],
            plain: table_separator_line_glow(&col_w),
            code_ref: None,
        });
    }
    for row in body_rows {
        out.extend(layout_table_row_glow(
            row,
            &col_w,
            &table.aligns,
            theme,
            false,
        ));
    }

    if prefix_spans.is_empty() {
        return out;
    }
    out.into_iter()
        .map(|mut line| {
            let mut spans = prefix_spans.clone();
            spans.extend(line.spans);
            line.plain = join_spans_plain(&spans);
            line.spans = spans;
            line
        })
        .collect()
}

fn table_separator_line_glow(col_w: &[u16]) -> String {
    let mut s = String::new();
    for (i, w) in col_w.iter().copied().enumerate() {
        if i > 0 {
            s.push('┼');
        }
        s.push_str(&"─".repeat(w.saturating_add(2) as usize));
    }
    s
}

fn layout_table_row_glow(
    row: &[Vec<Segment>],
    col_w: &[u16],
    aligns: &[Alignment],
    theme: &Theme,
    is_header: bool,
) -> Vec<RenderedLine> {
    let cols = col_w.len();

    let mut cells_wrapped: Vec<Vec<Vec<Span<'static>>>> = Vec::with_capacity(cols);
    let mut row_h = 1usize;
    for (ci, w) in col_w.iter().copied().enumerate() {
        let cell = row.get(ci).map(Vec::as_slice).unwrap_or(&[]);
        if is_header {
            // Glow keeps table headers single-line and truncates if needed.
            let spans = segments_to_spans(cell, theme);
            let max = w as usize;
            let spans = truncate_spans_with_ellipsis(spans, max, theme.text_primary);
            cells_wrapped.push(vec![spans]);
        } else {
            let lines = wrap_segments(&[], &[], cell, w, theme);
            let mut spans_lines: Vec<Vec<Span<'static>>> =
                lines.into_iter().map(|l| l.spans).collect();
            if spans_lines.is_empty() {
                spans_lines.push(vec![Span::raw("")]);
            }
            row_h = row_h.max(spans_lines.len());
            cells_wrapped.push(spans_lines);
        }
    }
    if is_header {
        row_h = 1;
    }

    let mut out: Vec<RenderedLine> = Vec::new();
    for li in 0..row_h {
        let mut spans: Vec<Span<'static>> = Vec::new();
        for ci in 0..cols {
            if ci > 0 {
                spans.push(Span::styled("│".to_string(), theme.text_muted));
            }

            let mut cell_spans = cells_wrapped[ci]
                .get(li)
                .cloned()
                .unwrap_or_else(|| vec![Span::raw("")]);

            if is_header {
                for s in &mut cell_spans {
                    s.style = s.style.add_modifier(Modifier::BOLD);
                }
            }

            let align = aligns.get(ci).copied().unwrap_or(Alignment::Left);
            let cell_spans = pad_spans(cell_spans, col_w[ci], align, theme.text_primary);
            spans.push(Span::styled(" ".to_string(), theme.text_primary));
            spans.extend(cell_spans);
            spans.push(Span::styled(" ".to_string(), theme.text_primary));
        }
        let plain = join_spans_plain(&spans);
        out.push(RenderedLine {
            spans,
            plain,
            code_ref: None,
        });
    }

    out
}

fn truncate_spans_with_ellipsis(
    spans: Vec<Span<'static>>,
    max_cols: usize,
    fallback_style: Style,
) -> Vec<Span<'static>> {
    if max_cols == 0 {
        return Vec::new();
    }

    let mut out: Vec<Span<'static>> = Vec::new();
    let mut cols = 0usize;
    let mut truncated = false;

    'outer: for span in spans {
        let mut buf = String::new();
        for ch in span.content.as_ref().chars() {
            let w = UnicodeWidthChar::width(ch).unwrap_or(0);
            if w == 0 {
                continue;
            }
            if cols + w > max_cols {
                truncated = true;
                break 'outer;
            }
            cols += w;
            buf.push(ch);
        }
        if !buf.is_empty() {
            out.push(Span::styled(buf, span.style));
        }
        if cols >= max_cols {
            break;
        }
    }

    if !truncated {
        // Still ensure we don't exceed max, but no ellipsis needed.
        return out;
    }

    // Replace the last visible cell char with an ellipsis when possible.
    if max_cols == 1 {
        return vec![Span::styled("…".to_string(), fallback_style)];
    }

    // Remove content until we have room for '…' (width=1).
    while cols >= max_cols {
        if let Some(last) = out.pop() {
            let text = last.content.to_string();
            let mut removed = 0usize;
            let mut new = String::new();
            for ch in text.chars() {
                let w = UnicodeWidthChar::width(ch).unwrap_or(0);
                if w == 0 {
                    continue;
                }
                new.push(ch);
                removed += w;
            }
            cols = cols.saturating_sub(removed);
        } else {
            break;
        }
    }

    // Trim last span to fit (max_cols - 1), then append ellipsis.
    let target = max_cols.saturating_sub(1);
    let mut trimmed: Vec<Span<'static>> = Vec::new();
    let mut cur = 0usize;
    for span in out {
        let mut buf = String::new();
        for ch in span.content.as_ref().chars() {
            let w = UnicodeWidthChar::width(ch).unwrap_or(0);
            if w == 0 {
                continue;
            }
            if cur + w > target {
                break;
            }
            cur += w;
            buf.push(ch);
        }
        if !buf.is_empty() {
            trimmed.push(Span::styled(buf, span.style));
        }
        if cur >= target {
            break;
        }
    }
    trimmed.push(Span::styled("…".to_string(), fallback_style));
    trimmed
}

fn layout_table_box(table: &TableBlock, width: u16, theme: &Theme) -> Vec<RenderedLine> {
    let prefix_spans = segments_to_spans(&table.prefix, theme);
    let prefix_cols = UnicodeWidthStr::width(join_segments_plain(&table.prefix).as_str()) as u16;
    let width = width.saturating_sub(prefix_cols);

    let cols = table
        .head
        .iter()
        .chain(table.body.iter())
        .map(|row| row.len())
        .max()
        .unwrap_or(0);
    if cols == 0 || width == 0 {
        return Vec::new();
    }

    let (head_rows, body_rows) = if !table.head.is_empty() {
        (table.head.as_slice(), table.body.as_slice())
    } else if !table.body.is_empty() {
        (&table.body[0..1], &table.body[1..])
    } else {
        (&[][..], &[][..])
    };

    // Box-drawing style similar to glamour/glow tables:
    // ┌────┬────┐
    // │ .. │ .. │
    // ├────┼────┤
    // │ .. │ .. │
    // └────┴────┘
    //
    // Each cell gets 1 space padding on both sides => effective width is (col_w + 2).
    // Total table width is: sum(col_w) + (3 * cols + 1)
    let chrome_w = 3u16.saturating_mul(cols as u16).saturating_add(1);
    let min_col_w = 3u16;

    let mut col_w: Vec<u16> = vec![min_col_w; cols];
    for row in table.head.iter().chain(table.body.iter()) {
        for (ci, cell) in row.iter().enumerate() {
            let w = UnicodeWidthStr::width(join_segments_plain(cell).as_str()) as u16;
            col_w[ci] = col_w[ci].max(w).max(1);
        }
    }

    let available = width.saturating_sub(chrome_w);
    let min_total = min_col_w.saturating_mul(cols as u16);
    if min_total > available {
        let per = (available / cols as u16).max(1);
        col_w.fill(per);
    } else {
        let mut total = col_w.iter().copied().sum::<u16>();
        while total > available {
            if let Some((idx, _)) = col_w
                .iter()
                .enumerate()
                .filter(|(_, w)| **w > min_col_w)
                .max_by_key(|(_, w)| **w)
            {
                col_w[idx] -= 1;
                total -= 1;
            } else {
                break;
            }
        }
    }

    let mut out: Vec<RenderedLine> = Vec::new();
    out.push(RenderedLine {
        spans: vec![Span::styled(
            table_border_line(&col_w, BorderLine::Top),
            theme.text_muted,
        )],
        plain: table_border_line(&col_w, BorderLine::Top),
        code_ref: None,
    });
    for row in head_rows {
        out.extend(layout_table_row(row, &col_w, &table.aligns, theme, true));
    }
    if !head_rows.is_empty() {
        out.push(RenderedLine {
            spans: vec![Span::styled(
                table_border_line(&col_w, BorderLine::HeaderSep),
                theme.text_muted,
            )],
            plain: table_border_line(&col_w, BorderLine::HeaderSep),
            code_ref: None,
        });
    }
    for row in body_rows {
        out.extend(layout_table_row(row, &col_w, &table.aligns, theme, false));
    }
    out.push(RenderedLine {
        spans: vec![Span::styled(
            table_border_line(&col_w, BorderLine::Bottom),
            theme.text_muted,
        )],
        plain: table_border_line(&col_w, BorderLine::Bottom),
        code_ref: None,
    });

    if prefix_spans.is_empty() {
        return out;
    }
    out.into_iter()
        .map(|mut line| {
            let mut spans = prefix_spans.clone();
            spans.extend(line.spans);
            line.plain = join_spans_plain(&spans);
            line.spans = spans;
            line
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
enum BorderLine {
    Top,
    HeaderSep,
    Bottom,
}

fn table_border_line(col_w: &[u16], kind: BorderLine) -> String {
    let mut s = String::new();
    for (i, w) in col_w.iter().copied().enumerate() {
        let seg = "─".repeat(w.saturating_add(2) as usize);
        if i > 0 {
            match kind {
                BorderLine::Top => s.push('┬'),
                BorderLine::HeaderSep => s.push('┼'),
                BorderLine::Bottom => s.push('┴'),
            }
        } else {
            match kind {
                BorderLine::Top => s.push('┌'),
                BorderLine::HeaderSep => s.push('├'),
                BorderLine::Bottom => s.push('└'),
            }
        }
        s.push_str(&seg);
    }
    match kind {
        BorderLine::Top => s.push('┐'),
        BorderLine::HeaderSep => s.push('┤'),
        BorderLine::Bottom => s.push('┘'),
    }
    s
}

fn layout_table_row(
    row: &[Vec<Segment>],
    col_w: &[u16],
    aligns: &[Alignment],
    theme: &Theme,
    is_header: bool,
) -> Vec<RenderedLine> {
    let cols = col_w.len();
    let mut cells_wrapped: Vec<Vec<Vec<Span<'static>>>> = Vec::with_capacity(cols);
    let mut row_h = 1usize;
    for (ci, w) in col_w.iter().copied().enumerate() {
        let cell = row.get(ci).map(Vec::as_slice).unwrap_or(&[]);
        let lines = wrap_segments(&[], &[], cell, w, theme);
        let mut spans_lines: Vec<Vec<Span<'static>>> = lines.into_iter().map(|l| l.spans).collect();
        if spans_lines.is_empty() {
            spans_lines.push(vec![Span::raw("")]);
        }
        row_h = row_h.max(spans_lines.len());
        cells_wrapped.push(spans_lines);
    }

    let mut out: Vec<RenderedLine> = Vec::new();
    for li in 0..row_h {
        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.push(Span::styled("│".to_string(), theme.text_muted));
        for ci in 0..cols {
            spans.push(Span::styled(" ".to_string(), theme.text_primary));
            let mut cell_spans = cells_wrapped[ci]
                .get(li)
                .cloned()
                .unwrap_or_else(|| vec![Span::raw("")]);
            if is_header {
                for s in &mut cell_spans {
                    s.style = s.style.add_modifier(Modifier::BOLD);
                }
            }
            let align = aligns.get(ci).copied().unwrap_or(Alignment::Left);
            let cell_spans = pad_spans(cell_spans, col_w[ci], align, theme.text_primary);
            spans.extend(cell_spans);
            spans.push(Span::styled(" ".to_string(), theme.text_primary));
            spans.push(Span::styled("│".to_string(), theme.text_muted));
        }
        let plain = join_spans_plain(&spans);
        out.push(RenderedLine {
            spans,
            plain,
            code_ref: None,
        });
    }
    out
}

fn pad_spans(
    mut spans: Vec<Span<'static>>,
    width: u16,
    align: Alignment,
    pad_style: Style,
) -> Vec<Span<'static>> {
    let w = UnicodeWidthStr::width(join_spans_plain(&spans).as_str()) as i32;
    let target = width as i32;
    let pad = (target - w).max(0) as usize;
    let (left, right) = match align {
        Alignment::Left => (0, pad),
        Alignment::Right => (pad, 0),
        Alignment::Center => (pad / 2, pad - pad / 2),
        Alignment::None => (0, pad),
    };
    if left > 0 {
        spans.insert(0, Span::styled(" ".repeat(left), pad_style));
    }
    if right > 0 {
        spans.push(Span::styled(" ".repeat(right), pad_style));
    }
    spans
}

fn wrap_segments(
    initial_prefix: &[Segment],
    subsequent_prefix: &[Segment],
    segments: &[Segment],
    width: u16,
    theme: &Theme,
) -> Vec<RenderedLine> {
    if width == 0 {
        return Vec::new();
    }
    let width = width as usize;

    let mut tokens: Vec<Segment> = Vec::new();
    for seg in segments {
        tokens.extend(split_segment_ws(seg));
    }

    let mut out: Vec<RenderedLine> = Vec::new();
    let mut cur: Vec<Segment> = Vec::new();
    let mut cur_cols: usize;

    let push_line = |out: &mut Vec<RenderedLine>, cur: &mut Vec<Segment>, prefix: &[Segment]| {
        let mut spans = segments_to_spans(prefix, theme);
        spans.extend(segments_to_spans(cur, theme));
        let plain = join_spans_plain(&spans);
        out.push(RenderedLine {
            spans,
            plain,
            code_ref: None,
        });
        cur.clear();
    };

    let mut prefix = initial_prefix.to_vec();
    let mut prefix_cols = UnicodeWidthStr::width(join_segments_plain(&prefix).as_str());
    cur_cols = prefix_cols;

    for tok in tokens {
        let tok_cols = UnicodeWidthStr::width(tok.text.as_str());
        if cur.is_empty() && is_all_ws(&tok.text) {
            continue;
        }

        if cur_cols + tok_cols <= width {
            cur.push(tok);
            cur_cols += tok_cols;
            continue;
        }

        if cur.is_empty() {
            let mut remaining = tok;
            loop {
                if remaining.text.is_empty() || is_all_ws(&remaining.text) {
                    break;
                }
                let remaining_cols = UnicodeWidthStr::width(remaining.text.as_str());
                if cur_cols + remaining_cols <= width {
                    cur.push(remaining);
                    cur_cols += remaining_cols;
                    break;
                }
                let max = width.saturating_sub(cur_cols).max(1);
                let (head, tail) = split_to_width_prefer_url_breaks(&remaining, max);
                cur.push(head);
                push_line(&mut out, &mut cur, &prefix);
                prefix = subsequent_prefix.to_vec();
                prefix_cols = UnicodeWidthStr::width(join_segments_plain(&prefix).as_str());
                cur_cols = prefix_cols;
                remaining = tail;
            }
            continue;
        }

        push_line(&mut out, &mut cur, &prefix);
        prefix = subsequent_prefix.to_vec();
        prefix_cols = UnicodeWidthStr::width(join_segments_plain(&prefix).as_str());
        cur_cols = prefix_cols;

        if is_all_ws(&tok.text) {
            continue;
        }
        let mut remaining = tok;
        loop {
            if remaining.text.is_empty() || is_all_ws(&remaining.text) {
                break;
            }
            let remaining_cols = UnicodeWidthStr::width(remaining.text.as_str());
            if cur_cols + remaining_cols <= width {
                cur.push(remaining);
                cur_cols += remaining_cols;
                break;
            }
            let max = width.saturating_sub(cur_cols).max(1);
            let (head, tail) = split_to_width_prefer_url_breaks(&remaining, max);
            cur.push(head);
            push_line(&mut out, &mut cur, &prefix);
            prefix = subsequent_prefix.to_vec();
            prefix_cols = UnicodeWidthStr::width(join_segments_plain(&prefix).as_str());
            cur_cols = prefix_cols;
            remaining = tail;
        }
    }

    if !cur.is_empty() || !prefix.is_empty() {
        push_line(&mut out, &mut cur, &prefix);
    }

    out
}

fn split_to_width_prefer_url_breaks(seg: &Segment, max_cols: usize) -> (Segment, Segment) {
    if looks_like_url(&seg.text)
        && let Some(split_idx) = last_url_breakpoint_before(&seg.text, max_cols)
    {
        let (a, b) = seg.text.split_at(split_idx);
        let mut left = seg.clone();
        left.text = a.to_string();
        let mut right = seg.clone();
        right.text = b.to_string();
        return (left, right);
    }
    split_to_width(seg, max_cols)
}

fn looks_like_url(s: &str) -> bool {
    s.starts_with("https://") || s.starts_with("http://")
}

fn last_url_breakpoint_before(s: &str, max_cols: usize) -> Option<usize> {
    if max_cols == 0 {
        return None;
    }
    let mut cols = 0usize;
    let mut best: Option<usize> = None;
    for (byte_idx, ch) in s.char_indices() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w == 0 {
            continue;
        }
        if cols + w > max_cols {
            break;
        }
        cols += w;
        if is_url_break_char(ch) {
            best = Some(byte_idx + ch.len_utf8());
        }
    }
    best
}

fn is_url_break_char(ch: char) -> bool {
    matches!(ch, '.' | '-' | '_' | '~' | '?' | '&' | '#' | '=')
}

fn split_segment_ws(seg: &Segment) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    let mut buf = String::new();
    let mut last_was_ws: Option<bool> = None;
    for ch in seg.text.chars() {
        let is_ws = ch.is_whitespace();
        match last_was_ws {
            None => {
                buf.push(ch);
                last_was_ws = Some(is_ws);
            }
            Some(prev) if prev == is_ws => buf.push(ch),
            Some(_) => {
                let mut s = seg.clone();
                s.text = std::mem::take(&mut buf);
                out.push(s);
                buf.push(ch);
                last_was_ws = Some(is_ws);
            }
        }
    }
    if !buf.is_empty() {
        let mut s = seg.clone();
        s.text = buf;
        out.push(s);
    }
    out
}

fn split_to_width(seg: &Segment, max_cols: usize) -> (Segment, Segment) {
    if max_cols == 0 {
        let mut left = seg.clone();
        left.text.clear();
        return (left, seg.clone());
    }
    let mut cols = 0usize;
    let mut idx = 0usize;
    for (byte_idx, ch) in seg.text.char_indices() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w == 0 {
            continue;
        }
        if cols + w > max_cols {
            break;
        }
        cols += w;
        idx = byte_idx + ch.len_utf8();
    }
    let (a, b) = seg.text.split_at(idx);
    let mut left = seg.clone();
    left.text = a.to_string();
    let mut right = seg.clone();
    right.text = b.to_string();
    (left, right)
}

fn is_all_ws(s: &str) -> bool {
    s.chars().all(|c| c.is_whitespace())
}

fn segments_to_spans(segs: &[Segment], theme: &Theme) -> Vec<Span<'static>> {
    segs.iter()
        .filter(|s| !s.text.is_empty())
        .map(|s| Span::styled(s.text.clone(), style_for_segment(theme, s)))
        .collect()
}

fn style_for_segment(theme: &Theme, seg: &Segment) -> Style {
    let mut style = if seg.muted {
        theme.text_muted
    } else {
        match seg.style {
            ProseStyle::Normal => theme.text_primary,
            ProseStyle::Heading(1) => theme
                .text_primary
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ProseStyle::Heading(_) => theme.text_primary.add_modifier(Modifier::BOLD),
            ProseStyle::BlockQuote => theme.text_muted,
            ProseStyle::List => theme.text_muted,
        }
    };

    if seg.inline_code {
        style = theme.code_inline;
    }
    if seg.link {
        style = theme.accent.add_modifier(Modifier::UNDERLINED);
    }
    if seg.flags.emphasis {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if seg.flags.strong {
        style = style.add_modifier(Modifier::BOLD);
    }
    if seg.flags.strike {
        style = style.add_modifier(Modifier::CROSSED_OUT);
    }

    style
}

fn join_segments_plain(segs: &[Segment]) -> String {
    let mut out = String::new();
    for s in segs {
        out.push_str(&s.text);
    }
    out
}

fn join_spans_plain(spans: &[Span<'static>]) -> String {
    let mut out = String::new();
    for s in spans {
        out.push_str(s.content.as_ref());
    }
    out
}

fn patch_spans_style(mut spans: Vec<Span<'static>>, base: Style) -> Vec<Span<'static>> {
    for s in &mut spans {
        s.style = base.patch(s.style);
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    #[test]
    fn parses_code_block_language_and_lines() {
        let md = "```rs\nfn main() {}\n```\n";
        let blocks = parse_markdown_blocks(
            md,
            ParseOptions {
                preserve_new_lines: false,
                show_link_destinations: false,
                show_heading_markers: false,
                glow_compat_relative_paths: false,
                link_destination_style: LinkDestinationStyle::Paren,
                glow_compat_quote_list_wrap: true,
                glow_compat_loose_list_join: false,
                glow_compat_post_list_blank_lines: 0,
                footnotes_at_end: false,
                blockquote_prefix: "| ",
                code_block_indent: 4,
                code_block_indent_in_blockquote: 2,
                footnote_hanging_indent: true,
                base_url: None,
            },
        );
        let code = blocks
            .iter()
            .find_map(|b| match b {
                Block::Code(c) => Some(c),
                _ => None,
            })
            .unwrap();
        assert_eq!(code.language.as_deref(), Some("rs"));
        assert_eq!(code.lines.as_slice(), &["fn main() {}".to_string()]);
    }

    #[test]
    fn wraps_prose_but_not_code() {
        let md = "hello world\n\n```txt\nabcdef\n```\n";
        let mut view = MarkdownView::new();
        view.set_markdown(md);
        let theme = Theme::default();
        view.ensure_layout(5, &theme);
        assert!(view.rendered.iter().any(|l| l.plain == "hello"));
        assert!(view.rendered.iter().any(|l| l.plain == "world"));
        assert!(view.rendered.iter().any(|l| l.plain == "    abcdef"));
    }

    #[test]
    fn can_show_line_numbers_for_code_blocks() {
        let md = "```rs\nfn main() {}\n```\n";
        let theme = Theme::default();
        let mut view = MarkdownView::with_options(MarkdownViewOptions {
            show_code_line_numbers: true,
            ..MarkdownViewOptions::default()
        });
        view.set_markdown(md);
        view.ensure_layout(80, &theme);
        let rendered = view
            .rendered
            .iter()
            .map(|l| l.plain.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("1 │ fn main() {}"));
    }

    #[test]
    fn parses_table_block() {
        let md = "| a | b |\n|---|---|\n| 1 | 2 |\n";
        let blocks = parse_markdown_blocks(
            md,
            ParseOptions {
                preserve_new_lines: false,
                show_link_destinations: false,
                show_heading_markers: false,
                glow_compat_relative_paths: false,
                link_destination_style: LinkDestinationStyle::Paren,
                glow_compat_quote_list_wrap: true,
                glow_compat_loose_list_join: false,
                glow_compat_post_list_blank_lines: 0,
                footnotes_at_end: false,
                blockquote_prefix: "| ",
                code_block_indent: 4,
                code_block_indent_in_blockquote: 2,
                footnote_hanging_indent: true,
                base_url: None,
            },
        );
        assert!(blocks.iter().any(|b| matches!(b, Block::Table(_))));
    }

    #[test]
    fn renders_tables_with_borders() {
        let md = "| a | b |\n|---|---|\n| 1 | 2 |\n";
        let mut view = MarkdownView::with_options(MarkdownViewOptions {
            table_style: TableStyle::Box,
            ..MarkdownViewOptions::default()
        });
        view.set_markdown(md);
        let theme = Theme::default();
        view.ensure_layout(80, &theme);
        let rendered = view
            .rendered
            .iter()
            .map(|l| l.plain.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains('┌'));
        assert!(rendered.contains('┐'));
        assert!(rendered.contains('│'));
        assert!(rendered.contains('└'));
        assert!(rendered.contains('┘'));
    }

    #[test]
    fn renders_tables_in_glow_style_by_default() {
        let md = "| a | b |\n|---|---|\n| 1 | 2 |\n";
        let mut view = MarkdownView::new();
        view.set_markdown(md);
        let theme = Theme::default();
        view.ensure_layout(40, &theme);
        let rendered = view
            .rendered
            .iter()
            .map(|l| l.plain.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains('│'));
        assert!(rendered.contains('┼'));
        assert!(rendered.contains('─'));
        assert!(!rendered.contains('┌'));
        assert!(rendered.contains("a │ b"));
    }

    #[test]
    fn task_list_marker_replaces_bullet() {
        let md = "- [x] done\n- [ ] todo\n";
        let mut view = MarkdownView::new();
        view.set_markdown(md);
        let theme = Theme::default();
        view.ensure_layout(40, &theme);
        let rendered = view
            .rendered
            .iter()
            .map(|l| l.plain.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("[✓]"));
        assert!(rendered.contains("[ ]"));
    }

    #[test]
    fn nested_list_items_are_indented_under_parent_item() {
        let md = "- item 2\n  - nested item\n";
        let mut view = MarkdownView::new();
        view.set_markdown(md);
        let theme = Theme::default();
        view.ensure_layout(80, &theme);
        let rendered = view
            .rendered
            .iter()
            .map(|l| l.plain.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("• item 2"));
        assert!(rendered.contains("  • nested item"));
        assert!(!rendered.contains("• • nested"));
    }

    #[test]
    fn loose_list_second_paragraph_is_indented_not_re_bulleted() {
        let md = "- item A\n\n  second paragraph.\n";
        let mut view = MarkdownView::new();
        view.set_markdown(md);
        let theme = Theme::default();
        view.ensure_layout(80, &theme);
        let lines = view
            .rendered
            .iter()
            .map(|l| l.plain.clone())
            .collect::<Vec<_>>();
        assert!(lines.iter().any(|l| l.contains("• item A")));
        assert!(lines.iter().any(|l| l.trim_end() == "  second paragraph."));
        assert!(!lines.iter().any(|l| l.contains("• second paragraph")));
    }

    #[test]
    fn renders_horizontal_rule_like_glow() {
        let md = "---\n";
        let mut view = MarkdownView::new();
        view.set_markdown(md);
        let theme = Theme::default();
        view.ensure_layout(80, &theme);
        let rendered = view
            .rendered
            .iter()
            .map(|l| l.plain.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("--------"));
    }

    #[test]
    fn renders_blockquote_like_glow() {
        let md = "> quote\n";
        let mut view = MarkdownView::new();
        view.set_markdown(md);
        let theme = Theme::default();
        view.ensure_layout(80, &theme);
        let rendered = view
            .rendered
            .iter()
            .map(|l| l.plain.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("| quote"));
    }

    #[test]
    fn glow_compat_quote_list_wrap_drops_bar_on_continuation() {
        let md = "> - a list inside quote\n";
        let mut view = MarkdownView::new();
        view.set_markdown(md);
        let theme = Theme::default();
        view.ensure_layout(12, &theme);
        let lines = view
            .rendered
            .iter()
            .map(|l| l.plain.clone())
            .collect::<Vec<_>>();
        assert!(lines.iter().any(|l| l.contains("| •")));
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("  ") && l.contains("inside") && !l.contains('|'))
        );
    }

    #[test]
    fn strips_inline_html_tags_like_glow() {
        let md = "<kbd>Ctrl</kbd> + <kbd>C</kbd>\n";
        let mut view = MarkdownView::new();
        view.set_markdown(md);
        let theme = Theme::default();
        view.ensure_layout(80, &theme);
        let rendered = view
            .rendered
            .iter()
            .map(|l| l.plain.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("Ctrl + C"));
        assert!(!rendered.contains("<kbd>"));
    }

    #[test]
    fn renders_images_like_glow() {
        let md = "![Glow](https://github.com/charmbracelet/glow)\n";
        let mut view = MarkdownView::new();
        view.set_markdown(md);
        let theme = Theme::default();
        view.ensure_layout(80, &theme);
        let rendered = view
            .rendered
            .iter()
            .map(|l| l.plain.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("Image:"));
        assert!(rendered.contains("Glow"));
        assert!(rendered.contains("→"));
        assert!(rendered.contains("https://github.com/charmbracelet/glow"));
    }

    #[test]
    fn renders_footnote_definitions() {
        let md = "Footnote[^a].\n\n[^a]: definition\n";
        let mut view = MarkdownView::new();
        view.set_markdown(md);
        let theme = Theme::default();
        view.ensure_layout(80, &theme);
        let rendered = view
            .rendered
            .iter()
            .map(|l| l.plain.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("[^a]:"));
    }

    #[test]
    fn indents_nested_code_blocks() {
        let md = "- item\n\n  ```txt\n  code\n  ```\n";
        let mut view = MarkdownView::new();
        view.set_markdown(md);
        let theme = Theme::default();
        view.ensure_layout(80, &theme);
        let rendered = view
            .rendered
            .iter()
            .map(|l| l.plain.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("  code"));
    }

    #[test]
    fn renders_without_panicking() {
        let md = "# Title\n\n- item one\n- item two\n";
        let mut view = MarkdownView::new();
        view.set_markdown(md);
        let theme = Theme::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 5));
        view.render_ref(Rect::new(0, 0, 30, 5), &mut buf, &theme);
    }

    #[test]
    fn link_destination_display_is_optional() {
        let md = "This is a [link](https://example.com).\n";
        let theme = Theme::default();

        let mut view = MarkdownView::with_options(MarkdownViewOptions {
            show_link_destinations: false,
            ..MarkdownViewOptions::default()
        });
        view.set_markdown(md);
        view.ensure_layout(80, &theme);
        let plain = view
            .rendered
            .iter()
            .map(|l| l.plain.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!plain.contains("https://example.com"));

        let mut view = MarkdownView::with_options(MarkdownViewOptions {
            show_link_destinations: true,
            ..MarkdownViewOptions::default()
        });
        view.set_markdown(md);
        view.ensure_layout(80, &theme);
        let plain = view
            .rendered
            .iter()
            .map(|l| l.plain.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(plain.contains("(https://example.com)"));
    }

    #[test]
    fn shows_destination_for_reference_links() {
        let md = "Reference link: [glow][glow-ref]\n\n[glow-ref]: https://github.com/charmbracelet/glow\n";
        let theme = Theme::default();
        let mut view = MarkdownView::with_options(MarkdownViewOptions {
            show_link_destinations: true,
            ..MarkdownViewOptions::default()
        });
        view.set_markdown(md);
        view.ensure_layout(80, &theme);
        let plain = view
            .rendered
            .iter()
            .map(|l| l.plain.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(plain.contains("(https://github.com/charmbracelet/glow)"));
    }

    #[test]
    fn can_collect_footnote_definitions_at_end() {
        let md = "Before.\n\n[^a]: Definition early.\n\nAfter.\n";
        let theme = Theme::default();
        let mut view = MarkdownView::with_options(MarkdownViewOptions {
            footnotes_at_end: true,
            ..MarkdownViewOptions::default()
        });
        view.set_markdown(md);
        view.ensure_layout(80, &theme);
        let plain = view
            .rendered
            .iter()
            .map(|l| l.plain.clone())
            .collect::<Vec<_>>()
            .join("\n");
        let after = plain.find("After.").unwrap();
        let def = plain.find("[^a]:").unwrap();
        assert!(after < def);
    }

    #[test]
    fn resolves_relative_link_destinations_with_base_url() {
        let md = "This is a [link](./a/b).\n";
        let theme = Theme::default();
        let mut view = MarkdownView::with_options(MarkdownViewOptions {
            base_url: Some("https://example.com/docs/".to_string()),
            show_link_destinations: true,
            ..MarkdownViewOptions::default()
        });
        view.set_markdown(md);
        view.ensure_layout(80, &theme);
        let rendered = view
            .rendered
            .iter()
            .map(|l| l.plain.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("(https://example.com/docs/a/b)"));
    }

    #[test]
    fn resolves_relative_images_with_base_url() {
        let md = "![Alt](img/logo.png)\n";
        let theme = Theme::default();
        let mut view = MarkdownView::with_options(MarkdownViewOptions {
            base_url: Some("https://example.com/docs/".to_string()),
            ..MarkdownViewOptions::default()
        });
        view.set_markdown(md);
        view.ensure_layout(80, &theme);
        let rendered = view
            .rendered
            .iter()
            .map(|l| l.plain.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("https://example.com/docs/img/logo.png"));
    }

    #[test]
    fn caches_code_block_highlighting_across_layouts() {
        #[derive(Default)]
        struct CountingHighlighter {
            calls: AtomicUsize,
        }

        impl CodeHighlighter for CountingHighlighter {
            fn highlight_lines(
                &self,
                _language: Option<&str>,
                lines: &[&str],
            ) -> Vec<Vec<Span<'static>>> {
                self.calls.fetch_add(1, Ordering::SeqCst);
                lines
                    .iter()
                    .map(|l| vec![Span::raw((*l).to_string())])
                    .collect()
            }
        }

        let md = "```rs\nfn main() {}\n```\n";
        let theme = Theme::default();
        let highlighter = Arc::new(CountingHighlighter::default());

        let mut view = MarkdownView::new();
        view.set_markdown(md);
        view.set_highlighter(Some(highlighter.clone()));

        let _ = view.lines_for_width(80, &theme);
        let _ = view.lines_for_width(40, &theme);

        assert_eq!(highlighter.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn document_render_matches_view_lines_for_width() {
        let md = "# Title\n\nHello **world**.\n";
        let theme = Theme::default();

        let opts = document::MarkdownRenderOptions::default();
        let doc = document::MarkdownDocument::parse(md, &opts);
        let rendered = doc.render(40, &theme, &opts, None);

        let mut view = MarkdownView::new();
        view.set_markdown(md);
        let lines = view.lines_for_width(40, &theme);

        assert_eq!(rendered.text().lines.as_slice(), lines.as_slice());
    }

    #[test]
    fn document_skips_highlighting_when_over_limit() {
        #[derive(Default)]
        struct CountingHighlighter {
            calls: AtomicUsize,
        }

        impl CodeHighlighter for CountingHighlighter {
            fn highlight_lines(
                &self,
                _language: Option<&str>,
                lines: &[&str],
            ) -> Vec<Vec<Span<'static>>> {
                self.calls.fetch_add(1, Ordering::SeqCst);
                lines
                    .iter()
                    .map(|l| vec![Span::raw((*l).to_string())])
                    .collect()
            }
        }

        let md = "```rs\nline1\nline2\n```\n";
        let theme = Theme::default();
        let highlighter = Arc::new(CountingHighlighter::default());

        let mut opts = document::MarkdownRenderOptions::default();
        opts.max_highlight_lines = 1;

        let doc = document::MarkdownDocument::parse(md, &opts);
        let _ = doc.render(80, &theme, &opts, Some(highlighter.clone()));

        assert_eq!(highlighter.calls.load(Ordering::SeqCst), 0);
    }
}
