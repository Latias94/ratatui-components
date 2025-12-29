use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui_components::input::InputEvent;
use ratatui_components::input::KeyCode;
use ratatui_components::render;
use ratatui_components::text::CodeHighlighter;
use ratatui_components::theme::Theme;
use ratatui_components::viewport::ViewportState;
use ratatui_components_ansi::ansi_text;
use ratatui_components_diff::DiffView;
#[cfg(feature = "mdstream")]
use ratatui_components_markdown::streaming::MarkdownStreamView;
use ratatui_components_markdown::view::MarkdownView;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    Tool,
    System,
}

impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Role::User => "USER",
            Role::Assistant => "ASSISTANT",
            Role::Tool => "TOOL",
            Role::System => "SYSTEM",
        }
    }
}

#[derive(Clone, Debug)]
pub enum EntryContent {
    Markdown(String),
    Diff(String),
    Ansi(String),
    Plain(String),
}

#[derive(Clone, Debug)]
pub struct TranscriptEntry {
    pub role: Role,
    pub content: EntryContent,
}

#[derive(Clone, Debug)]
pub struct TranscriptViewOptions {
    pub show_scrollbar: bool,
    pub follow_tail: bool,
    pub max_entries: Option<usize>,
    pub max_total_lines: Option<u32>,
    pub cache_entries: usize,
    pub streaming_markdown_pending_code_fence_max_lines: Option<usize>,
}

impl Default for TranscriptViewOptions {
    fn default() -> Self {
        Self {
            show_scrollbar: true,
            follow_tail: true,
            max_entries: None,
            max_total_lines: None,
            cache_entries: 48,
            streaming_markdown_pending_code_fence_max_lines: Some(40),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct EntryMetrics {
    height: u16,
    max_width: u16, // content-only width (excludes gutter prefix)
}

#[derive(Default)]
struct LinesLru {
    cap: usize,
    order: VecDeque<usize>,
    map: HashMap<usize, Vec<Line<'static>>>,
}

impl LinesLru {
    fn with_capacity(cap: usize) -> Self {
        Self {
            cap,
            order: VecDeque::new(),
            map: HashMap::new(),
        }
    }

    fn clear(&mut self) {
        self.order.clear();
        self.map.clear();
    }

    fn remove(&mut self, idx: usize) {
        self.map.remove(&idx);
        if let Some(pos) = self.order.iter().position(|&x| x == idx) {
            self.order.remove(pos);
        }
    }

    fn remove_from(&mut self, start: usize) {
        let keys: Vec<usize> = self.map.keys().copied().filter(|&k| k >= start).collect();
        for k in keys {
            self.remove(k);
        }
    }

    fn set_capacity(&mut self, cap: usize) {
        self.cap = cap;
        self.evict();
    }

    fn get(&mut self, idx: usize) -> Option<&Vec<Line<'static>>> {
        if self.map.contains_key(&idx) {
            self.touch(idx);
        }
        self.map.get(&idx)
    }

    fn insert(&mut self, idx: usize, lines: Vec<Line<'static>>) {
        self.map.insert(idx, lines);
        self.touch(idx);
        self.evict();
    }

    fn touch(&mut self, idx: usize) {
        if let Some(pos) = self.order.iter().position(|&x| x == idx) {
            self.order.remove(pos);
        }
        self.order.push_back(idx);
    }

    fn evict(&mut self) {
        while self.cap > 0 && self.order.len() > self.cap {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            }
        }
        if self.cap == 0 {
            self.clear();
        }
    }
}

#[derive(Default)]
pub struct TranscriptView {
    entries: Vec<TranscriptEntry>,
    metrics: Vec<EntryMetrics>,
    offsets: Vec<u32>,         // len = entries.len() + 1
    cached_width: Option<u16>, // full content area width (incl. gutter)
    gutter_width: u16,         // label width only
    layout_dirty: bool,
    layout_dirty_from: Option<usize>,
    follow_tail_pinned: bool,
    force_to_bottom: bool,
    cache: LinesLru,
    pub state: ViewportState,
    options: TranscriptViewOptions,
    highlighter: Option<Arc<dyn CodeHighlighter + Send + Sync>>,
    #[cfg(feature = "mdstream")]
    streaming_markdown: Option<(usize, MarkdownStreamView)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranscriptAction {
    None,
    Redraw,
    FollowTailToggled(bool),
}

impl TranscriptView {
    pub fn new() -> Self {
        let mut v = Self::default();
        v.follow_tail_pinned = true;
        v.cache = LinesLru::with_capacity(v.options.cache_entries);
        v.layout_dirty = true;
        v.layout_dirty_from = None;
        v
    }

    pub fn with_options(options: TranscriptViewOptions) -> Self {
        let mut v = Self::default();
        v.options = options;
        v.follow_tail_pinned = true;
        v.cache = LinesLru::with_capacity(v.options.cache_entries);
        v.layout_dirty = true;
        v.layout_dirty_from = None;
        v
    }

    pub fn set_highlighter(&mut self, highlighter: Option<Arc<dyn CodeHighlighter + Send + Sync>>) {
        self.highlighter = highlighter;
        self.mdstream_on_set_highlighter();
        self.invalidate_layout();
    }

    pub fn follow_tail_enabled(&self) -> bool {
        self.options.follow_tail
    }

    pub fn set_follow_tail(&mut self, enabled: bool) {
        self.options.follow_tail = enabled;
        if enabled {
            self.follow_tail_pinned = true;
            self.force_to_bottom = true;
        }
        self.state.clamp();
    }

    pub fn set_streaming_markdown_pending_code_fence_max_lines(
        &mut self,
        max_lines: Option<usize>,
    ) {
        self.options.streaming_markdown_pending_code_fence_max_lines = max_lines;
        self.mdstream_on_set_pending_code_fence_max_lines(max_lines);
        self.invalidate_layout();
    }

    pub fn handle_event(&mut self, event: InputEvent) -> TranscriptAction {
        match event {
            InputEvent::Paste(_) => TranscriptAction::None,
            InputEvent::Key(key) => {
                if key.modifiers.ctrl || key.modifiers.alt {
                    if key.modifiers.ctrl && matches!(key.code, KeyCode::Char('d')) {
                        let delta = self.state.viewport_h.saturating_sub(1) as i32;
                        self.scroll_y_by(delta);
                        return TranscriptAction::Redraw;
                    }
                    if key.modifiers.ctrl && matches!(key.code, KeyCode::Char('u')) {
                        let delta = -(self.state.viewport_h.saturating_sub(1) as i32);
                        self.scroll_y_by(delta);
                        return TranscriptAction::Redraw;
                    }
                    return TranscriptAction::None;
                }

                match key.code {
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.scroll_y_by(1);
                        TranscriptAction::Redraw
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.scroll_y_by(-1);
                        TranscriptAction::Redraw
                    }
                    KeyCode::PageDown => {
                        let delta = self.state.viewport_h.saturating_sub(1) as i32;
                        self.scroll_y_by(delta);
                        TranscriptAction::Redraw
                    }
                    KeyCode::PageUp => {
                        let delta = -(self.state.viewport_h.saturating_sub(1) as i32);
                        self.scroll_y_by(delta);
                        TranscriptAction::Redraw
                    }
                    KeyCode::Home | KeyCode::Char('g') => {
                        self.state.to_top();
                        self.follow_tail_pinned = false;
                        TranscriptAction::Redraw
                    }
                    KeyCode::End | KeyCode::Char('G') => {
                        self.state.to_bottom();
                        self.follow_tail_pinned = true;
                        TranscriptAction::Redraw
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        self.scroll_x_by(-4);
                        TranscriptAction::Redraw
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        self.scroll_x_by(4);
                        TranscriptAction::Redraw
                    }
                    KeyCode::Char('f') => {
                        let enabled = !self.options.follow_tail;
                        self.set_follow_tail(enabled);
                        TranscriptAction::FollowTailToggled(enabled)
                    }
                    _ => TranscriptAction::None,
                }
            }
        }
    }

    pub fn push_markdown(&mut self, role: Role, markdown: &str) {
        self.push_entry(TranscriptEntry {
            role,
            content: EntryContent::Markdown(markdown.to_string()),
        });
    }

    pub fn push_diff(&mut self, role: Role, diff: &str) {
        self.push_entry(TranscriptEntry {
            role,
            content: EntryContent::Diff(diff.to_string()),
        });
    }

    pub fn push_ansi(&mut self, role: Role, ansi: &str) {
        self.push_entry(TranscriptEntry {
            role,
            content: EntryContent::Ansi(ansi.to_string()),
        });
    }

    pub fn push_plain(&mut self, role: Role, text: &str) {
        self.push_entry(TranscriptEntry {
            role,
            content: EntryContent::Plain(text.to_string()),
        });
    }

    pub fn push_entry(&mut self, entry: TranscriptEntry) {
        self.entries.push(entry);
        self.mdstream_on_push_entry();
        if self.options.follow_tail {
            self.force_to_bottom = self.follow_tail_pinned;
        }
        if let Some(max_entries) = self.options.max_entries
            && self.entries.len() > max_entries
        {
            let drop = self.entries.len().saturating_sub(max_entries);
            self.entries.drain(0..drop);
            self.mdstream_on_drop_entries();
            self.invalidate_layout();
            return;
        }
        self.invalidate_layout_from(self.entries.len().saturating_sub(1));
    }

    pub fn push_or_append_markdown(&mut self, role: Role, delta: &str) {
        if self.append_to_last_markdown(role, delta) {
            return;
        }
        self.push_markdown(role, delta);
    }

    pub fn append_to_last_markdown(&mut self, role: Role, delta: &str) -> bool {
        let idx = self.entries.len().saturating_sub(1);
        {
            let Some(last) = self.entries.last_mut() else {
                return false;
            };
            if last.role != role {
                return false;
            }
            let EntryContent::Markdown(s) = &mut last.content else {
                return false;
            };
            s.push_str(delta);
        }
        self.mdstream_on_append_to_last_markdown(idx, delta);
        if self.options.follow_tail {
            self.force_to_bottom = self.follow_tail_pinned;
        }
        self.invalidate_layout_from(self.entries.len().saturating_sub(1));
        true
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.invalidate_layout();
        self.state.to_top();
        self.state.clamp();
    }

    pub fn set_viewport(&mut self, area: Rect) {
        let content_area = if self.options.show_scrollbar && area.width >= 2 {
            Rect::new(area.x, area.y, area.width - 1, area.height)
        } else {
            area
        };
        self.ensure_layout(content_area.width, &Theme::default());
        let prefix_w = prefix_width(self.gutter_width).min(content_area.width);
        let content_w = content_area.width.saturating_sub(prefix_w);
        self.state.set_viewport(content_w, content_area.height);
        self.state.clamp();
    }

    pub fn scroll_y_by(&mut self, delta: i32) {
        self.state.scroll_y_by(delta);
        if delta < 0 {
            self.follow_tail_pinned = false;
        } else if self.is_at_bottom() {
            self.follow_tail_pinned = true;
        }
    }

    pub fn scroll_x_by(&mut self, delta: i32) {
        self.state.scroll_x_by(delta);
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

        let prefix_w = prefix_width(self.gutter_width).min(content_area.width);
        let visible_content_w = content_area.width.saturating_sub(prefix_w);
        self.state
            .set_viewport(visible_content_w, content_area.height);
        self.ensure_layout(content_area.width, theme);

        for row in 0..content_area.height {
            let y = content_area.y + row;
            let global = self.state.y + row as u32;

            buf.set_style(
                Rect::new(content_area.x, y, content_area.width, 1),
                theme.text_primary,
            );

            if global >= self.total_lines() {
                continue;
            }

            match self.locate(global) {
                Located::Spacer => {
                    let prefix = blank_prefix(self.gutter_width, theme);
                    render::render_spans_clipped(
                        content_area.x,
                        y,
                        0,
                        prefix_w,
                        buf,
                        &prefix,
                        theme.text_muted,
                    );
                }
                Located::Entry { idx, line } => {
                    let prefix =
                        make_prefix(self.entries[idx].role, self.gutter_width, line == 0, theme);
                    render::render_spans_clipped(
                        content_area.x,
                        y,
                        0,
                        prefix_w,
                        buf,
                        &prefix,
                        theme.text_muted,
                    );

                    if visible_content_w == 0 {
                        continue;
                    }

                    let lines = self.entry_lines(idx, content_area.width, theme);
                    let Some(content_line) = lines.get(line as usize) else {
                        continue;
                    };
                    render::render_spans_clipped(
                        content_area.x + prefix_w,
                        y,
                        self.state.x,
                        visible_content_w,
                        buf,
                        &content_line.spans,
                        theme.text_primary,
                    );
                }
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

    fn invalidate_layout(&mut self) {
        self.cached_width = None;
        self.layout_dirty = true;
        self.layout_dirty_from = None;
        self.metrics.clear();
        self.offsets.clear();
        self.cache.clear();
        self.state.set_content(0, 0);
        self.mdstream_on_invalidate_layout();
    }

    fn invalidate_layout_from(&mut self, idx: usize) {
        self.layout_dirty = true;
        self.layout_dirty_from = Some(self.layout_dirty_from.map_or(idx, |cur| cur.min(idx)));
        self.cache.remove_from(idx);
    }

    fn ensure_layout(&mut self, width: u16, theme: &Theme) {
        let new_gutter = self.compute_gutter_width();
        let gutter_changed = new_gutter != self.gutter_width;
        self.gutter_width = new_gutter;

        let width_changed = self.cached_width != Some(width);
        if width_changed {
            self.cached_width = Some(width);
            self.layout_dirty = true;
            self.layout_dirty_from = None;
        }
        if gutter_changed {
            self.layout_dirty = true;
            self.layout_dirty_from = None;
        }

        self.cache.set_capacity(self.options.cache_entries);

        if !self.layout_dirty {
            return;
        }

        self.layout_dirty = false;
        let full_rebuild = width_changed
            || gutter_changed
            || self.layout_dirty_from.is_none()
            || self.metrics.len() != self.entries.len()
            || self.offsets.len() != self.entries.len() + 1;
        if full_rebuild {
            self.cache.clear();
        }

        let prefix_w = prefix_width(self.gutter_width).min(width);
        let content_w = width.saturating_sub(prefix_w);

        if full_rebuild {
            self.metrics = Vec::with_capacity(self.entries.len());
            self.offsets = Vec::with_capacity(self.entries.len() + 1);

            let mut max_content_width: u16 = 0;
            for entry in &self.entries {
                let m = compute_entry_metrics(entry, content_w);
                max_content_width = max_content_width.max(m.max_width);
                self.metrics.push(m);
            }
            self.rebuild_offsets();

            if let Some(max_total) = self.options.max_total_lines {
                while self.total_lines() > max_total && !self.entries.is_empty() {
                    self.entries.remove(0);
                    if !self.metrics.is_empty() {
                        self.metrics.remove(0);
                    }
                    self.cache.clear();
                    self.rebuild_offsets();
                }
            }

            self.state
                .set_content(max_content_width as u32, self.total_lines());
        } else {
            let start_idx = self.layout_dirty_from.unwrap_or(self.entries.len());
            self.layout_dirty_from = None;
            if self.metrics.len() < self.entries.len() {
                self.metrics
                    .resize_with(self.entries.len(), EntryMetrics::default);
            }
            for idx in start_idx..self.entries.len() {
                let entry = &self.entries[idx];
                self.metrics[idx] = compute_entry_metrics(entry, content_w);
            }
            self.rebuild_offsets();
            let max_content_width = self.metrics.iter().map(|m| m.max_width).max().unwrap_or(0);

            if let Some(max_total) = self.options.max_total_lines {
                while self.total_lines() > max_total && !self.entries.is_empty() {
                    self.entries.remove(0);
                    if !self.metrics.is_empty() {
                        self.metrics.remove(0);
                    }
                    self.cache.clear();
                    self.layout_dirty_from = None;
                    self.rebuild_offsets();
                }
            }

            self.state
                .set_content(max_content_width as u32, self.total_lines());
        }

        if self.force_to_bottom {
            self.state.to_bottom();
            self.force_to_bottom = false;
        }
        if self.is_at_bottom() {
            self.follow_tail_pinned = true;
        }
        let _ = theme;
    }

    fn compute_gutter_width(&self) -> u16 {
        self.entries
            .iter()
            .map(|e| UnicodeWidthStr::width(e.role.label()) as u16)
            .max()
            .unwrap_or(0)
            .max(4)
    }

    fn rebuild_offsets(&mut self) {
        self.offsets.clear();
        let mut cur: u32 = 0;
        self.offsets.push(cur);
        for (i, m) in self.metrics.iter().enumerate() {
            cur = cur.saturating_add(m.height as u32);
            if i + 1 < self.metrics.len() {
                cur = cur.saturating_add(1);
            }
            self.offsets.push(cur);
        }
    }

    fn total_lines(&self) -> u32 {
        self.offsets.last().copied().unwrap_or(0)
    }

    fn max_y(&self) -> u32 {
        self.state
            .content_h
            .saturating_sub(self.state.viewport_h as u32)
    }

    fn is_at_bottom(&self) -> bool {
        self.state.y >= self.max_y()
    }

    fn locate(&self, global: u32) -> Located {
        let idx = upper_bound(&self.offsets, global).saturating_sub(1);
        let idx = idx.min(self.entries.len().saturating_sub(1));
        let start = self.offsets.get(idx).copied().unwrap_or(0);
        let height = self.metrics.get(idx).map(|m| m.height as u32).unwrap_or(0);
        let end = start + height;
        if global < end {
            Located::Entry {
                idx,
                line: (global - start) as u16,
            }
        } else {
            Located::Spacer
        }
    }

    fn entry_lines(&mut self, idx: usize, width: u16, theme: &Theme) -> Vec<Line<'static>> {
        if let Some(lines) = self.cache.get(idx) {
            return lines.clone();
        }

        let prefix_w = prefix_width(self.gutter_width).min(width);
        let content_w = width.saturating_sub(prefix_w);
        if let Some(lines) = self.mdstream_entry_lines(idx, content_w, theme) {
            self.cache.insert(idx, lines.clone());
            return lines;
        }

        let entry = self.entries.get(idx);
        let Some(entry) = entry else {
            return vec![Line::from("")];
        };

        let lines = render_entry_lines(entry, content_w, theme, self.highlighter.clone());
        self.cache.insert(idx, lines.clone());
        lines
    }
}

#[cfg(feature = "mdstream")]
impl TranscriptView {
    fn mdstream_on_set_highlighter(&mut self) {
        if let Some((_, view)) = self.streaming_markdown.as_mut() {
            view.set_highlighter(self.highlighter.clone());
        }
    }

    fn mdstream_on_set_pending_code_fence_max_lines(&mut self, max_lines: Option<usize>) {
        if let Some((_, view)) = self.streaming_markdown.as_mut() {
            view.set_pending_code_fence_max_lines(max_lines);
        }
    }

    fn mdstream_on_push_entry(&mut self) {
        self.streaming_markdown = None;
    }

    fn mdstream_on_drop_entries(&mut self) {
        self.streaming_markdown = None;
    }

    fn mdstream_on_invalidate_layout(&mut self) {
        self.streaming_markdown = None;
    }

    fn mdstream_on_append_to_last_markdown(&mut self, idx: usize, delta: &str) {
        let full = match self.entries.get(idx) {
            Some(TranscriptEntry {
                content: EntryContent::Markdown(s),
                ..
            }) => s.as_str(),
            _ => return,
        };
        match self.streaming_markdown.as_mut() {
            Some((i, v)) if *i == idx => {
                let _ = v.append(delta);
            }
            _ => {
                let mut v = MarkdownStreamView::default();
                v.set_highlighter(self.highlighter.clone());
                v.set_pending_code_fence_max_lines(
                    self.options.streaming_markdown_pending_code_fence_max_lines,
                );
                let _ = v.append(full);
                self.streaming_markdown = Some((idx, v));
            }
        }
    }

    fn mdstream_entry_lines(
        &mut self,
        idx: usize,
        content_w: u16,
        theme: &Theme,
    ) -> Option<Vec<Line<'static>>> {
        let (stream_idx, view) = self.streaming_markdown.as_mut()?;
        if *stream_idx != idx {
            return None;
        }
        Some(view.snapshot_lines(content_w, theme))
    }
}

#[cfg(not(feature = "mdstream"))]
impl TranscriptView {
    fn mdstream_on_set_highlighter(&mut self) {}
    fn mdstream_on_set_pending_code_fence_max_lines(&mut self, _max_lines: Option<usize>) {}
    fn mdstream_on_push_entry(&mut self) {}
    fn mdstream_on_drop_entries(&mut self) {}
    fn mdstream_on_invalidate_layout(&mut self) {}
    fn mdstream_on_append_to_last_markdown(&mut self, _idx: usize, _delta: &str) {}
    fn mdstream_entry_lines(
        &mut self,
        _idx: usize,
        _content_w: u16,
        _theme: &Theme,
    ) -> Option<Vec<Line<'static>>> {
        None
    }
}

fn prefix_width(gutter_width: u16) -> u16 {
    // "<LABEL padded> │ " = gutter_width + 3
    gutter_width.saturating_add(3)
}

fn make_prefix(role: Role, gutter_width: u16, first: bool, theme: &Theme) -> Vec<Span<'static>> {
    let role_style = role_style(theme, role);
    let sep_style = theme.text_muted;
    if first {
        vec![
            Span::styled(
                format!("{:>width$}", role.label(), width = gutter_width as usize),
                role_style,
            ),
            Span::styled(" │ ".to_string(), sep_style),
        ]
    } else {
        vec![
            Span::styled(" ".repeat(gutter_width as usize), sep_style),
            Span::styled(" │ ".to_string(), sep_style),
        ]
    }
}

fn blank_prefix(gutter_width: u16, theme: &Theme) -> Vec<Span<'static>> {
    let sep_style = theme.text_muted;
    vec![
        Span::styled(" ".repeat(gutter_width as usize), sep_style),
        Span::styled(" │ ".to_string(), sep_style),
    ]
}

fn role_style(theme: &Theme, role: Role) -> Style {
    match role {
        Role::User => theme.accent.add_modifier(Modifier::BOLD),
        Role::Assistant => theme.text_primary.add_modifier(Modifier::BOLD),
        Role::Tool => theme.text_muted.add_modifier(Modifier::BOLD),
        Role::System => theme.text_muted,
    }
}

struct TextWrap;

impl TextWrap {
    fn wrap_plain(input: &str, width: u16) -> Vec<Line<'static>> {
        if width == 0 {
            return vec![];
        }
        let width = width as usize;
        let mut out: Vec<Line<'static>> = Vec::new();
        for raw in input.lines() {
            let mut cur = String::new();
            for word in raw.split_whitespace() {
                let word_w = UnicodeWidthStr::width(word);
                if !cur.is_empty() && UnicodeWidthStr::width(cur.as_str()) + 1 + word_w > width {
                    out.push(Line::from(cur.clone()));
                    cur.clear();
                }
                if !cur.is_empty() {
                    cur.push(' ');
                }
                cur.push_str(word);
            }
            out.push(Line::from(cur));
        }
        out
    }
}

fn compute_entry_metrics(entry: &TranscriptEntry, content_width: u16) -> EntryMetrics {
    let lines = render_entry_lines(entry, content_width, &Theme::default(), None);
    let height = lines.len().min(u16::MAX as usize) as u16;
    let max_width = lines
        .iter()
        .map(|l| UnicodeWidthStr::width(spans_plain(&l.spans).as_str()) as u16)
        .max()
        .unwrap_or(0);
    EntryMetrics { height, max_width }
}

fn render_entry_lines(
    entry: &TranscriptEntry,
    content_width: u16,
    theme: &Theme,
    highlighter: Option<Arc<dyn CodeHighlighter + Send + Sync>>,
) -> Vec<Line<'static>> {
    match &entry.content {
        EntryContent::Markdown(md) => {
            let mut view = MarkdownView::new();
            view.set_markdown(md);
            view.set_highlighter(highlighter);
            view.lines_for_width(content_width, theme)
        }
        EntryContent::Diff(diff) => {
            let mut view = DiffView::new();
            view.set_diff(diff);
            view.set_highlighter(highlighter);
            view.lines_for_transcript(theme)
        }
        EntryContent::Ansi(s) => ansi_text(s).lines,
        EntryContent::Plain(s) => {
            let text = s.replace('\t', "    ");
            TextWrap::wrap_plain(&text, content_width)
        }
    }
}

fn spans_plain(spans: &[Span<'static>]) -> String {
    let mut out = String::new();
    for s in spans {
        out.push_str(s.content.as_ref());
    }
    out
}

#[derive(Clone, Copy, Debug)]
enum Located {
    Entry { idx: usize, line: u16 },
    Spacer,
}

fn upper_bound(sorted: &[u32], value: u32) -> usize {
    let mut lo = 0usize;
    let mut hi = sorted.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        if sorted[mid] <= value {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_components::input::KeyEvent;
    use ratatui_components::input::KeyModifiers;

    #[test]
    fn offsets_and_locate_work_with_spacers() {
        let mut tv = TranscriptView::with_options(TranscriptViewOptions {
            show_scrollbar: false,
            follow_tail: false,
            max_entries: None,
            max_total_lines: None,
            cache_entries: 0,
            streaming_markdown_pending_code_fence_max_lines: None,
        });
        tv.push_plain(Role::User, "one");
        tv.push_plain(Role::Assistant, "two\nthree");
        tv.ensure_layout(40, &Theme::default());
        assert_eq!(tv.entries.len(), 2);
        assert!(tv.total_lines() >= 3);

        match tv.locate(0) {
            Located::Entry { idx, line } => {
                assert_eq!(idx, 0);
                assert_eq!(line, 0);
            }
            _ => panic!("expected entry line"),
        }

        // there is a spacer between entries
        let spacer_line = tv.metrics[0].height as u32;
        assert!(matches!(tv.locate(spacer_line), Located::Spacer));
    }

    #[test]
    fn max_entries_trims_oldest() {
        let mut tv = TranscriptView::with_options(TranscriptViewOptions {
            show_scrollbar: false,
            follow_tail: false,
            max_entries: Some(2),
            max_total_lines: None,
            cache_entries: 0,
            streaming_markdown_pending_code_fence_max_lines: None,
        });
        tv.push_plain(Role::User, "1");
        tv.push_plain(Role::User, "2");
        tv.push_plain(Role::User, "3");
        tv.ensure_layout(40, &Theme::default());
        assert_eq!(tv.entries.len(), 2);
        assert!(matches!(
            &tv.entries[0].content,
            EntryContent::Plain(s) if s == "2"
        ));
    }

    #[test]
    fn max_total_lines_trims_until_under_limit() {
        let mut tv = TranscriptView::with_options(TranscriptViewOptions {
            show_scrollbar: false,
            follow_tail: false,
            max_entries: None,
            max_total_lines: Some(3),
            cache_entries: 0,
            streaming_markdown_pending_code_fence_max_lines: None,
        });
        tv.push_plain(Role::User, "one\ntwo\nthree");
        tv.push_plain(Role::Assistant, "x\ny\nz");
        tv.ensure_layout(40, &Theme::default());
        assert!(tv.total_lines() <= 3);
        assert!(!tv.entries.is_empty());
    }

    #[test]
    fn handle_event_toggle_follow_tail() {
        let mut tv = TranscriptView::new();
        assert!(tv.follow_tail_enabled());
        let act = tv.handle_event(InputEvent::Key(KeyEvent::new(KeyCode::Char('f'))));
        assert_eq!(act, TranscriptAction::FollowTailToggled(false));
        assert!(!tv.follow_tail_enabled());
        let act = tv.handle_event(InputEvent::Key(KeyEvent::new(KeyCode::Char('f'))));
        assert_eq!(act, TranscriptAction::FollowTailToggled(true));
        assert!(tv.follow_tail_enabled());
    }

    #[test]
    fn handle_event_ctrl_d_pages_down() {
        let mut tv = TranscriptView::new();
        tv.options.follow_tail = false;
        for i in 0..200 {
            tv.push_plain(Role::User, &format!("line {i}"));
        }
        tv.ensure_layout(40, &Theme::default());
        tv.state.set_viewport(20, 5);
        tv.state.to_top();
        let ev = KeyEvent::new(KeyCode::Char('d')).with_modifiers(KeyModifiers {
            shift: false,
            ctrl: true,
            alt: false,
        });
        let _ = tv.handle_event(InputEvent::Key(ev));
        assert!(tv.state.y > 0);
    }
}
