use crate::view::MarkdownView;
use crate::view::MarkdownViewOptions;
use mdstream::DocumentState;
use mdstream::MdStream;
use mdstream::Update;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui_components_core::render;
use ratatui_components_core::theme::Theme;
use ratatui_components_core::viewport::ViewportState;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;
use unicode_width::UnicodeWidthStr;

pub type MdStreamOptions = mdstream::Options;

/// A small helper that keeps `mdstream` state and can produce a UI-friendly Markdown string.
///
/// This is mainly useful for “agent CLI” streaming (token/chunk deltas) where you want stable
/// behavior for incomplete Markdown (e.g. open code fences).
#[derive(Debug)]
pub struct MarkdownStream {
    options: MdStreamOptions,
    stream: MdStream,
    state: DocumentState,
    display: String,
    display_dirty: bool,
}

impl Default for MarkdownStream {
    fn default() -> Self {
        Self::new(MdStreamOptions::default())
    }
}

impl MarkdownStream {
    pub fn new(options: MdStreamOptions) -> Self {
        Self {
            stream: MdStream::new(options.clone()),
            options,
            state: DocumentState::new(),
            display: String::new(),
            display_dirty: true,
        }
    }

    pub fn append(&mut self, delta: &str) -> Update {
        let u = self.stream.append(delta);
        self.apply_update(&u);
        u
    }

    pub fn finalize(&mut self) -> Update {
        let u = self.stream.finalize();
        self.apply_update(&u);
        u
    }

    pub fn reset(&mut self) {
        self.stream = MdStream::new(self.options.clone());
        self.state = DocumentState::new();
        self.display.clear();
        self.display_dirty = true;
    }

    pub fn committed(&self) -> &[mdstream::Block] {
        self.state.committed()
    }

    pub fn pending(&self) -> Option<&mdstream::Block> {
        self.state.pending()
    }

    pub fn display_markdown(&mut self) -> &str {
        if !self.display_dirty {
            return &self.display;
        }

        self.display.clear();
        for b in self.state.committed() {
            self.display.push_str(&b.raw);
        }
        if let Some(p) = self.state.pending() {
            self.display
                .push_str(p.display.as_deref().unwrap_or(&p.raw));
        }

        self.display_dirty = false;
        &self.display
    }

    fn apply_update(&mut self, u: &Update) {
        let applied = self.state.apply(u.clone());
        if applied.reset || !u.committed.is_empty() || u.pending.is_some() {
            self.display_dirty = true;
        }
    }
}

pub struct MarkdownStreamView {
    mdstream_options: MdStreamOptions,
    stream: MdStream,
    state: DocumentState,

    render_options: MarkdownViewOptions,
    engine: MarkdownView,

    cached_width: Option<u16>,

    committed_rendered_blocks: usize,
    committed_rendered_lines: Vec<Line<'static>>,
    committed_max_w: u16,

    pending_key: u64,
    pending_lines: Vec<Line<'static>>,
    pending_max_w: u16,

    pending_code_fence_max_lines: Option<usize>,

    pub viewport: ViewportState,
}

impl Default for MarkdownStreamView {
    fn default() -> Self {
        Self::with_options(MdStreamOptions::default(), MarkdownViewOptions::default())
    }
}

impl MarkdownStreamView {
    pub fn with_options(
        mdstream_options: MdStreamOptions,
        render_options: MarkdownViewOptions,
    ) -> Self {
        let mut engine_opts = render_options.clone();
        engine_opts.show_scrollbar = false;
        engine_opts.padding_left = 0;
        engine_opts.padding_right = 0;
        let mut engine = MarkdownView::with_options(engine_opts);
        engine.set_highlighter(None);

        Self {
            mdstream_options: mdstream_options.clone(),
            stream: MdStream::new(mdstream_options),
            state: DocumentState::new(),
            render_options,
            engine,
            cached_width: None,
            committed_rendered_blocks: 0,
            committed_rendered_lines: Vec::new(),
            committed_max_w: 0,
            pending_key: 0,
            pending_lines: Vec::new(),
            pending_max_w: 0,
            pending_code_fence_max_lines: None,
            viewport: ViewportState::default(),
        }
    }

    pub fn set_pending_code_fence_max_lines(&mut self, max_lines: Option<usize>) {
        self.pending_code_fence_max_lines = max_lines;
        self.pending_key = 0;
    }

    pub fn set_highlighter(
        &mut self,
        highlighter: Option<Arc<dyn ratatui_components_core::text::CodeHighlighter + Send + Sync>>,
    ) {
        self.engine.set_highlighter(highlighter);
        self.reset_layout_cache();
    }

    pub fn set_render_options(&mut self, options: MarkdownViewOptions) {
        self.render_options = options.clone();

        let mut engine_opts = options;
        engine_opts.show_scrollbar = false;
        engine_opts.padding_left = 0;
        engine_opts.padding_right = 0;
        self.engine = MarkdownView::with_options(engine_opts);

        self.reset_layout_cache();
    }

    pub fn append(&mut self, delta: &str) -> Update {
        let u = self.stream.append(delta);
        self.apply_update(&u);
        u
    }

    pub fn finalize(&mut self) -> Update {
        let u = self.stream.finalize();
        self.apply_update(&u);
        u
    }

    pub fn reset(&mut self) {
        self.stream = MdStream::new(self.mdstream_options.clone());
        self.state = DocumentState::new();
        self.reset_layout_cache();
    }

    pub fn committed(&self) -> &[mdstream::Block] {
        self.state.committed()
    }

    pub fn pending(&self) -> Option<&mdstream::Block> {
        self.state.pending()
    }

    pub fn render_ref(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let (content_area, scrollbar_x) = if self.render_options.show_scrollbar && area.width >= 2 {
            (
                Rect::new(area.x, area.y, area.width - 1, area.height),
                Some(area.x + area.width - 1),
            )
        } else {
            (area, None)
        };

        let inner = inset_h(
            content_area,
            self.render_options.padding_left,
            self.render_options.padding_right,
        );

        self.viewport.set_viewport(inner.width, inner.height);
        self.ensure_layout(inner.width, theme);

        for row in 0..content_area.height {
            let y = content_area.y + row;
            buf.set_style(
                Rect::new(content_area.x, y, content_area.width, 1),
                theme.text_primary,
            );
            let idx = (self.viewport.y as usize).saturating_add(row as usize);
            if let Some(line) = self.line_at(idx) {
                render::render_spans_clipped(
                    inner.x,
                    y,
                    self.viewport.x,
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
                &self.viewport,
                theme.text_muted,
            );
        }
    }

    pub fn total_lines_for_width(&mut self, width: u16, theme: &Theme) -> usize {
        self.ensure_layout(width, theme);
        self.committed_rendered_lines
            .len()
            .saturating_add(self.pending_lines.len())
    }

    pub fn snapshot_lines(&mut self, width: u16, theme: &Theme) -> Vec<Line<'static>> {
        self.ensure_layout(width, theme);
        let mut out = self.committed_rendered_lines.clone();
        out.extend(self.pending_lines.clone());
        out
    }

    fn ensure_layout(&mut self, width: u16, theme: &Theme) {
        let width_changed = self.cached_width != Some(width);
        if width_changed {
            self.reset_layout_cache();
        }
        self.cached_width = Some(width);

        if width == 0 {
            self.viewport.set_content(0, 0);
            return;
        }

        while self.committed_rendered_blocks < self.state.committed().len() {
            let b = &self.state.committed()[self.committed_rendered_blocks];
            let raw = b.raw.clone();
            let lines = self.render_md_block(&raw, width, theme);
            self.committed_max_w = self
                .committed_max_w
                .max(max_line_width(&lines).min(u16::MAX as usize) as u16);
            self.committed_rendered_lines.extend(lines);
            self.committed_rendered_blocks += 1;
        }

        let pending_key = self.pending_block_key();
        if pending_key != self.pending_key {
            self.pending_key = pending_key;
            self.pending_lines.clear();
            self.pending_max_w = 0;

            if let Some(p) = self.state.pending() {
                let mut md = p.display.as_deref().unwrap_or(&p.raw).to_string();
                if p.kind == mdstream::BlockKind::CodeFence
                    && let Some(max_lines) = self.pending_code_fence_max_lines
                {
                    md = truncate_pending_code_fence(&md, max_lines);
                }
                self.pending_lines = self.render_md_block(&md, width, theme);
                self.pending_max_w =
                    max_line_width(&self.pending_lines).min(u16::MAX as usize) as u16;
            }
        }

        let content_h = self
            .committed_rendered_lines
            .len()
            .saturating_add(self.pending_lines.len()) as u32;
        let content_w = self.committed_max_w.max(self.pending_max_w) as u32;
        self.viewport.set_content(content_w, content_h);
    }

    fn render_md_block(&mut self, markdown: &str, width: u16, theme: &Theme) -> Vec<Line<'static>> {
        self.engine.set_markdown(markdown);
        self.engine.lines_for_width(width, theme)
    }

    fn pending_block_key(&self) -> u64 {
        let Some(p) = self.state.pending() else {
            return 0;
        };
        let mut h = std::collections::hash_map::DefaultHasher::new();
        p.id.hash(&mut h);
        std::mem::discriminant(&p.kind).hash(&mut h);
        p.raw.hash(&mut h);
        if let Some(d) = &p.display {
            d.hash(&mut h);
        }
        h.finish()
    }

    fn apply_update(&mut self, u: &Update) {
        let applied = self.state.apply(u.clone());
        if applied.reset {
            self.reset_layout_cache();
        }
    }

    fn line_at(&self, idx: usize) -> Option<&Line<'static>> {
        if idx < self.committed_rendered_lines.len() {
            return self.committed_rendered_lines.get(idx);
        }
        let idx = idx.saturating_sub(self.committed_rendered_lines.len());
        self.pending_lines.get(idx)
    }

    fn reset_layout_cache(&mut self) {
        self.cached_width = None;
        self.committed_rendered_blocks = 0;
        self.committed_rendered_lines.clear();
        self.committed_max_w = 0;
        self.pending_key = 0;
        self.pending_lines.clear();
        self.pending_max_w = 0;
        self.viewport = ViewportState::default();
    }
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

fn line_width(line: &Line<'_>) -> usize {
    line.spans
        .iter()
        .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
        .sum()
}

fn max_line_width(lines: &[Line<'static>]) -> usize {
    lines.iter().map(line_width).max().unwrap_or(0)
}

fn truncate_pending_code_fence(display: &str, max_lines: usize) -> String {
    if max_lines == 0 {
        return String::new();
    }

    let mut lines = display.split_inclusive('\n').collect::<Vec<_>>();
    if lines.len() <= 2 {
        return display.to_string();
    }

    let opening = lines[0];
    let (indent, marker_ch, marker_len) = parse_fence_opening(opening).unwrap_or(("", '`', 3));
    let closing_line = format!("{indent}{}\n", marker_ch.to_string().repeat(marker_len));

    if let Some(last) = lines.last().copied()
        && is_fence_closing(last, indent, marker_ch, marker_len)
    {
        lines.pop();
    }

    let content_lines = &lines[1..];
    if content_lines.len() <= max_lines {
        let mut out = String::new();
        out.push_str(opening);
        out.extend(content_lines.iter().copied());
        out.push_str(&closing_line);
        return out;
    }

    let tail = &content_lines[content_lines.len() - max_lines..];
    let mut out = String::new();
    out.push_str(opening);
    out.push_str(indent);
    out.push_str("… generating more …\n");
    out.extend(tail.iter().copied());
    out.push_str(&closing_line);
    out
}

fn parse_fence_opening(opening: &str) -> Option<(&str, char, usize)> {
    let bytes = opening.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() && bytes[i] != b'\n' {
        i += 1;
    }
    let rest = &opening[i..];
    let ch = rest.chars().next()?;
    if ch != '`' && ch != '~' {
        return None;
    }
    let len = rest.chars().take_while(|c| *c == ch).count();
    if len < 3 {
        return None;
    }
    Some((&opening[..i], ch, len))
}

fn is_fence_closing(line: &str, indent: &str, ch: char, len: usize) -> bool {
    let trimmed = line.trim_end_matches(['\n', '\r']);
    let Some(rest) = trimmed.strip_prefix(indent) else {
        return false;
    };
    let mut chars = rest.chars();
    for _ in 0..len {
        if chars.next() != Some(ch) {
            return false;
        }
    }
    true
}
