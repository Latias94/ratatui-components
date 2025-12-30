mod parser;

use parser::DiffLineKind;
use parser::ParsedDiff;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use similar::DiffTag;
use similar::TextDiff;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use crate::input::MouseButton;
use crate::input::MouseEvent;
use crate::input::MouseEventKind;
use crate::render;
use crate::scroll::ScrollBindings;
use crate::selection::SelectionAction;
use crate::selection::SelectionBindings;
use crate::text::CodeHighlighter;
use crate::theme::Theme;
use crate::viewport::ViewportState;

/// Options for [`DiffView`].
///
/// This view is optimized for unified diffs (like `git diff` output). It supports:
/// - scrolling
/// - optional line numbers
/// - optional syntax highlighting for code-like lines
/// - mouse drag selection + copy-on-request
#[derive(Clone, Debug)]
pub struct DiffViewOptions {
    pub show_line_numbers: bool,
    pub show_scrollbar: bool,
    pub highlight_hunks: bool,
    pub highlight_inline_changes: bool,
    pub scroll: ScrollBindings,
    pub enable_selection: bool,
    pub selection: SelectionBindings,
}

impl Default for DiffViewOptions {
    fn default() -> Self {
        Self {
            show_line_numbers: true,
            show_scrollbar: true,
            highlight_hunks: true,
            highlight_inline_changes: true,
            scroll: ScrollBindings::default(),
            enable_selection: true,
            selection: SelectionBindings::default(),
        }
    }
}

/// A scrollable unified-diff viewer with optional syntax highlighting and selection + copy.
///
/// - The diff input is parsed via [`set_diff`](Self::set_diff).
/// - The view is event-loop agnostic: call `handle_event_action*` and `render_ref` from your app.
/// - Copy is app-controlled: a copy keybinding emits [`SelectionAction::CopyRequested`].
#[derive(Default)]
pub struct DiffView {
    parsed: ParsedDiff,
    pub state: ViewportState,
    options: DiffViewOptions,
    highlighter: Option<Arc<dyn CodeHighlighter + Send + Sync>>,
    language_override: Option<String>,
    inline_ranges: HashMap<usize, Vec<(usize, usize)>>,
    visible_highlight_cache: Option<VisibleHighlightCache>,
    highlight_scratch: String,
    selection_anchor: Option<(usize, u32)>,
    selection: Option<((usize, u32), (usize, u32))>,
}

#[derive(Clone, Debug)]
struct VisibleHighlightCache {
    start: usize,
    end: usize,
    hash: u64,
    spans: Arc<HashMap<usize, Vec<Span<'static>>>>,
}

impl Clone for DiffView {
    fn clone(&self) -> Self {
        Self {
            parsed: self.parsed.clone(),
            state: self.state,
            options: self.options.clone(),
            highlighter: self.highlighter.clone(),
            language_override: self.language_override.clone(),
            inline_ranges: self.inline_ranges.clone(),
            visible_highlight_cache: None,
            highlight_scratch: String::new(),
            selection_anchor: self.selection_anchor,
            selection: self.selection,
        }
    }
}

impl DiffView {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_options(options: DiffViewOptions) -> Self {
        Self {
            options,
            ..Default::default()
        }
    }

    /// Sets a highlighter used for visible lines.
    pub fn set_highlighter(&mut self, highlighter: Option<Arc<dyn CodeHighlighter + Send + Sync>>) {
        self.highlighter = highlighter;
        self.invalidate_highlighting();
    }

    /// Overrides language detection for highlighting.
    pub fn set_language_override(&mut self, language: Option<impl Into<String>>) {
        self.language_override = language.map(Into::into);
        self.invalidate_highlighting();
    }

    /// Parses a unified diff string and updates internal layout metrics.
    pub fn set_diff(&mut self, diff: &str) {
        self.parsed = parser::parse_unified_diff(diff);
        self.inline_ranges = if self.options.highlight_inline_changes {
            compute_inline_ranges(&self.parsed)
        } else {
            HashMap::new()
        };
        self.invalidate_highlighting();
        self.state.set_content(
            self.parsed.max_content_width as u32,
            self.parsed.lines.len() as u32,
        );
    }

    /// Updates viewport size for `area` (and accounts for optional gutter/scrollbar).
    pub fn set_viewport(&mut self, area: Rect) {
        let (old_w, new_w, gutter_w) = if self.options.show_line_numbers {
            let old_w = digits(self.parsed.max_old_lineno).max(1);
            let new_w = digits(self.parsed.max_new_lineno).max(1);
            let gutter_w = old_w + 1 + new_w + 1 + 1 + 1;
            (old_w as u16, new_w as u16, gutter_w as u16)
        } else {
            (0, 0, 2)
        };
        let _ = (old_w, new_w);

        let content_area = if self.options.show_scrollbar && area.width >= 2 {
            Rect::new(area.x, area.y, area.width - 1, area.height)
        } else {
            area
        };

        let viewport_w = content_area.width.saturating_sub(gutter_w);
        self.state.set_viewport(viewport_w, content_area.height);
    }

    /// Scrolls vertically (y axis).
    pub fn scroll_y_by(&mut self, delta: i32) {
        self.state.scroll_y_by(delta);
    }

    /// Scrolls horizontally (x axis).
    pub fn scroll_x_by(&mut self, delta: i32) {
        self.state.scroll_x_by(delta);
    }

    /// Handles an event and returns `true` if a redraw is needed.
    pub fn handle_event(&mut self, event: crate::input::InputEvent) -> bool {
        !matches!(self.handle_event_action(event), SelectionAction::None)
    }

    /// Handles an event and returns a [`SelectionAction`] (redraw / copy-on-request).
    pub fn handle_event_action(&mut self, event: crate::input::InputEvent) -> SelectionAction {
        match event {
            crate::input::InputEvent::Paste(_) => SelectionAction::None,
            crate::input::InputEvent::Mouse(_) => SelectionAction::None,
            crate::input::InputEvent::Key(key) => {
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

    /// Like [`Self::handle_event`], but first updates viewport state for `area`.
    pub fn handle_event_in_area(&mut self, area: Rect, event: crate::input::InputEvent) -> bool {
        !matches!(
            self.handle_event_action_in_area(area, event),
            SelectionAction::None
        )
    }

    /// Like [`Self::handle_event_action`], but first updates viewport state for `area`.
    pub fn handle_event_action_in_area(
        &mut self,
        area: Rect,
        event: crate::input::InputEvent,
    ) -> SelectionAction {
        match event {
            crate::input::InputEvent::Paste(_) => SelectionAction::None,
            crate::input::InputEvent::Key(_) => self.handle_event_action(event),
            crate::input::InputEvent::Mouse(m) => {
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
        if self.parsed.lines.is_empty() {
            return false;
        }

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

        let content_area = if self.options.show_scrollbar && area.width >= 2 {
            Rect::new(area.x, area.y, area.width - 1, area.height)
        } else {
            area
        };

        let gutter_w = if self.options.show_line_numbers {
            let old_w = digits(self.parsed.max_old_lineno).max(1);
            let new_w = digits(self.parsed.max_new_lineno).max(1);
            let gutter_w = old_w + 1 + new_w + 1 + 1 + 1;
            (gutter_w as u16).min(content_area.width)
        } else {
            2
        };

        let content_start_x = content_area.x.saturating_add(gutter_w);
        let content_end_x = content_area
            .x
            .saturating_add(content_area.width)
            .saturating_sub(1);
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

        let rel_x = (x - content_start_x) as u32;
        let rel_y = (y - content_area.y) as u32;
        let line = self
            .state
            .y
            .saturating_add(rel_y)
            .min(self.parsed.lines.len().saturating_sub(1) as u32) as usize;
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
                if self.selection_anchor.is_none() {
                    return false;
                }
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

    pub fn selected_text(&self) -> Option<String> {
        let ((l0, c0), (l1, c1)) = self.selection?;
        let ((start_line, start_col), (end_line, end_col)) =
            normalize_sel_inclusive((l0, c0), (l1, c1));

        let mut out = String::new();
        for line_idx in start_line..=end_line {
            let line = self.parsed.lines.get(line_idx)?;
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
            if let Some((bs, be)) = render::byte_range_for_cols(&line.content, from, to) {
                out.push_str(&line.content[bs..be]);
            }
        }
        Some(out)
    }

    pub fn render_ref(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        self.set_viewport(area);

        let code_bg = self.highlighter.as_ref().and_then(|h| h.background_color());

        let (content_area, scrollbar_x) = if self.options.show_scrollbar && area.width >= 2 {
            (
                Rect::new(area.x, area.y, area.width - 1, area.height),
                Some(area.x + area.width - 1),
            )
        } else {
            (area, None)
        };

        let (old_w, new_w, gutter_w) = if self.options.show_line_numbers {
            let old_w = digits(self.parsed.max_old_lineno).max(1);
            let new_w = digits(self.parsed.max_new_lineno).max(1);
            let gutter_w = old_w + 1 + new_w + 1 + 1 + 1;
            (old_w as u16, new_w as u16, gutter_w as u16)
        } else {
            (0, 0, 2)
        };

        let content_w = content_area.width.saturating_sub(gutter_w);

        let start = self.state.y as usize;
        let end = (start + content_area.height as usize).min(self.parsed.lines.len());

        let highlighted = if self.options.highlight_hunks
            && self.highlighter.is_some()
            && content_w > 0
            && start < end
        {
            Some(self.highlight_visible_cached(start, end))
        } else {
            None
        };

        for row in 0..content_area.height {
            let y = content_area.y + row;
            let idx = (self.state.y as usize).saturating_add(row as usize);
            let Some(line) = self.parsed.lines.get(idx) else {
                let style = if let Some(bg) = code_bg {
                    theme.text_primary.bg(bg)
                } else {
                    theme.text_primary
                };
                buf.set_style(Rect::new(content_area.x, y, content_area.width, 1), style);
                continue;
            };

            let line_style = style_for_kind(theme, line.kind);
            let line_style = if let Some(bg) = code_bg {
                line_style.bg(bg)
            } else {
                line_style
            };
            buf.set_style(
                Rect::new(content_area.x, y, content_area.width, 1),
                line_style,
            );

            let (old_str, new_str) = if self.options.show_line_numbers {
                (
                    line.old_lineno
                        .map(|n| format!("{n:>width$}", width = old_w as usize))
                        .unwrap_or_else(|| " ".repeat(old_w as usize)),
                    line.new_lineno
                        .map(|n| format!("{n:>width$}", width = new_w as usize))
                        .unwrap_or_else(|| " ".repeat(new_w as usize)),
                )
            } else {
                (String::new(), String::new())
            };

            let marker = marker_for_kind(line.kind);
            let gutter = if self.options.show_line_numbers {
                format!("{old_str} {new_str} {marker} ")
            } else {
                format!("{marker} ")
            };

            let gutter_style = gutter_style_for_kind(theme, line.kind);
            let gutter_style = if let Some(bg) = code_bg {
                gutter_style.bg(bg)
            } else {
                gutter_style
            };
            buf.set_stringn(
                content_area.x,
                y,
                &gutter,
                content_area.width as usize,
                gutter_style,
            );

            if content_w == 0 {
                continue;
            }

            match line.kind {
                DiffLineKind::Add | DiffLineKind::Del | DiffLineKind::Context => {
                    let mut spans = highlighted
                        .as_ref()
                        .and_then(|m| m.get(&idx))
                        .cloned()
                        .map(|s| patch_spans_style(s, line_style))
                        .unwrap_or_else(|| vec![Span::styled(line.content.clone(), line_style)]);
                    if self.options.highlight_inline_changes
                        && let Some(ranges) = self.inline_ranges.get(&idx)
                    {
                        spans = render::apply_modifier_to_byte_ranges(
                            spans,
                            ranges,
                            Modifier::REVERSED,
                        );
                    }
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
                                render::byte_range_for_cols(&line.content, from, to)
                            {
                                spans = render::apply_modifier_to_byte_ranges(
                                    spans,
                                    &[(bs, be)],
                                    Modifier::REVERSED,
                                );
                            }
                        }
                    }
                    render::render_spans_clipped(
                        content_area.x + gutter_w,
                        y,
                        self.state.x,
                        content_w,
                        buf,
                        &spans,
                        line_style,
                    );
                }
                _ => {
                    render::render_str_clipped(
                        content_area.x + gutter_w,
                        y,
                        self.state.x,
                        content_w,
                        buf,
                        &line.content,
                        line_style,
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

    pub fn as_text(&self, theme: &Theme) -> Text<'static> {
        let mut lines = Vec::with_capacity(self.parsed.lines.len());
        for l in &self.parsed.lines {
            let style = style_for_kind(theme, l.kind);
            lines.push(Line::from(vec![Span::styled(l.raw.clone(), style)]));
        }
        Text::from(lines)
    }

    pub fn lines_for_transcript(&mut self, theme: &Theme) -> Vec<Line<'static>> {
        let mut out: Vec<Line<'static>> = Vec::with_capacity(self.parsed.lines.len());
        let highlighted = if self.options.highlight_hunks && self.highlighter.is_some() {
            Some(self.highlight_visible_cached(0, self.parsed.lines.len()))
        } else {
            None
        };

        for (idx, l) in self.parsed.lines.iter().enumerate() {
            let line_style = style_for_kind(theme, l.kind);
            let line = match l.kind {
                DiffLineKind::Add | DiffLineKind::Del | DiffLineKind::Context => {
                    let prefix = l.raw.chars().next().unwrap_or(' ');
                    let mut spans: Vec<Span<'static>> = Vec::new();
                    spans.push(Span::styled(
                        prefix.to_string(),
                        gutter_style_for_kind(theme, l.kind),
                    ));

                    let mut rest = highlighted
                        .as_ref()
                        .and_then(|m| m.get(&idx))
                        .cloned()
                        .map(|s| patch_spans_style(s, line_style))
                        .unwrap_or_else(|| vec![Span::styled(l.content.clone(), line_style)]);
                    if self.options.highlight_inline_changes
                        && let Some(ranges) = self.inline_ranges.get(&idx)
                    {
                        rest =
                            render::apply_modifier_to_byte_ranges(rest, ranges, Modifier::REVERSED);
                    }
                    spans.extend(rest);
                    Line::from(spans)
                }
                _ => Line::from(vec![Span::styled(l.raw.clone(), line_style)]),
            };
            out.push(line);
        }

        out
    }

    fn highlight_visible_cached(
        &mut self,
        start: usize,
        end: usize,
    ) -> Arc<HashMap<usize, Vec<Span<'static>>>> {
        let hash =
            highlight_inputs_hash(&self.parsed, self.language_override.as_deref(), start, end);
        if let Some(cache) = self.visible_highlight_cache.as_ref()
            && cache.start == start
            && cache.end == end
            && cache.hash == hash
        {
            return cache.spans.clone();
        }

        let spans = Arc::new(self.highlight_visible_uncached(start, end));
        self.visible_highlight_cache = Some(VisibleHighlightCache {
            start,
            end,
            hash,
            spans: spans.clone(),
        });
        spans
    }

    fn highlight_visible_uncached(
        &mut self,
        start: usize,
        end: usize,
    ) -> HashMap<usize, Vec<Span<'static>>> {
        let mut out: HashMap<usize, Vec<Span<'static>>> = HashMap::new();
        let Some(hi) = self.highlighter.as_ref() else {
            return out;
        };

        let mut run_lang: Option<&str> = None;
        let mut run_lines: Vec<&str> = Vec::new();
        let mut run_indices: Vec<usize> = Vec::new();

        let flush = |out: &mut HashMap<usize, Vec<Span<'static>>>,
                     scratch: &mut String,
                     lang: &mut Option<&str>,
                     lines: &mut Vec<&str>,
                     indices: &mut Vec<usize>| {
            if indices.is_empty() {
                return;
            }
            scratch.clear();
            for (i, line) in lines.iter().enumerate() {
                if i > 0 {
                    scratch.push('\n');
                }
                scratch.push_str(line);
            }
            let highlighted = hi.highlight_text(*lang, scratch);
            for (i, idx) in indices.iter().copied().enumerate() {
                let spans = highlighted.get(i).cloned().unwrap_or_default();
                out.insert(idx, spans);
            }
            lines.clear();
            indices.clear();
        };

        for idx in start..end {
            let Some(line) = self.parsed.lines.get(idx) else {
                continue;
            };
            if !matches!(
                line.kind,
                DiffLineKind::Context | DiffLineKind::Add | DiffLineKind::Del
            ) {
                flush(
                    &mut out,
                    &mut self.highlight_scratch,
                    &mut run_lang,
                    &mut run_lines,
                    &mut run_indices,
                );
                continue;
            }
            let lang = self
                .language_override
                .as_deref()
                .or(line.language_hint.as_deref());
            if run_lang != lang {
                flush(
                    &mut out,
                    &mut self.highlight_scratch,
                    &mut run_lang,
                    &mut run_lines,
                    &mut run_indices,
                );
                run_lang = lang;
            }
            run_lines.push(line.content.as_str());
            run_indices.push(idx);
        }
        flush(
            &mut out,
            &mut self.highlight_scratch,
            &mut run_lang,
            &mut run_lines,
            &mut run_indices,
        );
        out
    }

    fn invalidate_highlighting(&mut self) {
        self.visible_highlight_cache = None;
    }
}

fn highlight_inputs_hash(
    parsed: &ParsedDiff,
    language_override: Option<&str>,
    start: usize,
    end: usize,
) -> u64 {
    let mut h = DefaultHasher::new();
    language_override.unwrap_or("").hash(&mut h);
    for idx in start..end {
        let Some(line) = parsed.lines.get(idx) else {
            continue;
        };
        match line.kind {
            DiffLineKind::Context | DiffLineKind::Add | DiffLineKind::Del => {
                let lang = language_override
                    .or(line.language_hint.as_deref())
                    .unwrap_or("");
                lang.hash(&mut h);
                kind_tag(line.kind).hash(&mut h);
                line.content.hash(&mut h);
            }
            _ => {
                kind_tag(line.kind).hash(&mut h);
            }
        }
    }
    h.finish()
}

fn kind_tag(kind: DiffLineKind) -> u8 {
    match kind {
        DiffLineKind::FileHeader => 1,
        DiffLineKind::HunkHeader => 2,
        DiffLineKind::Add => 3,
        DiffLineKind::Del => 4,
        DiffLineKind::Context => 5,
        DiffLineKind::Meta => 6,
    }
}

fn digits(n: u32) -> usize {
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

fn marker_for_kind(kind: DiffLineKind) -> char {
    match kind {
        DiffLineKind::HunkHeader => '@',
        DiffLineKind::Add => '+',
        DiffLineKind::Del => '-',
        DiffLineKind::Context => ' ',
        DiffLineKind::FileHeader => ' ',
        DiffLineKind::Meta => ' ',
    }
}

fn style_for_kind(theme: &Theme, kind: DiffLineKind) -> Style {
    match kind {
        DiffLineKind::Add => theme.diff_add,
        DiffLineKind::Del => theme.diff_del,
        DiffLineKind::HunkHeader => theme.accent.add_modifier(Modifier::BOLD),
        DiffLineKind::FileHeader => theme.text_muted,
        DiffLineKind::Context | DiffLineKind::Meta => theme.text_primary,
    }
}

fn gutter_style_for_kind(theme: &Theme, kind: DiffLineKind) -> Style {
    match kind {
        DiffLineKind::Add => theme.diff_add,
        DiffLineKind::Del => theme.diff_del,
        _ => theme.text_muted,
    }
}

fn patch_spans_style(mut spans: Vec<Span<'static>>, base: Style) -> Vec<Span<'static>> {
    for s in &mut spans {
        s.style = base.patch(s.style);
    }
    spans
}

fn compute_inline_ranges(parsed: &ParsedDiff) -> HashMap<usize, Vec<(usize, usize)>> {
    let mut out: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();
    let mut i = 0usize;
    while i < parsed.lines.len() {
        if parsed.lines[i].kind != DiffLineKind::Del {
            i += 1;
            continue;
        }

        let mut dels: Vec<usize> = Vec::new();
        while i < parsed.lines.len() && parsed.lines[i].kind == DiffLineKind::Del {
            dels.push(i);
            i += 1;
        }

        let mut adds: Vec<usize> = Vec::new();
        while i < parsed.lines.len() && parsed.lines[i].kind == DiffLineKind::Add {
            adds.push(i);
            i += 1;
        }

        let pairs = dels.len().min(adds.len());
        for k in 0..pairs {
            let del_idx = dels[k];
            let add_idx = adds[k];
            let del = &parsed.lines[del_idx].content;
            let add = &parsed.lines[add_idx].content;
            let diff = TextDiff::from_chars(del, add);
            let del_map = char_start_indices(del);
            let add_map = char_start_indices(add);
            for op in diff.ops() {
                if op.tag() == DiffTag::Equal {
                    continue;
                }
                let old = op.old_range();
                if old.start < old.end {
                    push_range(&mut out, del_idx, &del_map, old);
                }
                let new = op.new_range();
                if new.start < new.end {
                    push_range(&mut out, add_idx, &add_map, new);
                }
            }
        }
    }

    for v in out.values_mut() {
        v.sort_by_key(|(s, _)| *s);
        let mut merged: Vec<(usize, usize)> = Vec::with_capacity(v.len());
        for (s, e) in v.drain(..) {
            if let Some(last) = merged.last_mut()
                && s <= last.1
            {
                last.1 = last.1.max(e);
                continue;
            }
            merged.push((s, e));
        }
        *v = merged;
    }

    out
}

fn char_start_indices(s: &str) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::with_capacity(s.chars().count() + 1);
    for (idx, _) in s.char_indices() {
        out.push(idx);
    }
    out.push(s.len());
    out
}

fn push_range(
    out: &mut HashMap<usize, Vec<(usize, usize)>>,
    line_idx: usize,
    char_starts: &[usize],
    range: std::ops::Range<usize>,
) {
    let start = range.start.min(char_starts.len().saturating_sub(1));
    let end = range.end.min(char_starts.len().saturating_sub(1));
    let start_b = char_starts[start];
    let end_b = char_starts[end];
    if start_b >= end_b {
        return;
    }
    out.entry(line_idx).or_default().push((start_b, end_b));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::NoHighlight;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Color;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    #[test]
    fn digits_behaves() {
        assert_eq!(digits(0), 1);
        assert_eq!(digits(9), 1);
        assert_eq!(digits(10), 2);
        assert_eq!(digits(999), 3);
    }

    #[test]
    fn renders_without_panicking() {
        let diff = "\
diff --git a/a.txt b/a.txt
--- a/a.txt
+++ b/a.txt
@@ -1,1 +1,2 @@
 a
+b
";
        let mut view = DiffView::new();
        view.set_diff(diff);
        let theme = Theme::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 5));
        view.render_ref(Rect::new(0, 0, 40, 5), &mut buf, &theme);
    }

    #[test]
    fn renders_with_scrollbar_narrow_width() {
        let diff = "\
diff --git a/a.txt b/a.txt
@@ -1,1 +1,2 @@
 a
+b
";
        let mut view = DiffView::with_options(DiffViewOptions {
            show_line_numbers: true,
            show_scrollbar: true,
            highlight_hunks: true,
            highlight_inline_changes: true,
            ..Default::default()
        });
        view.set_diff(diff);
        let theme = Theme::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 2, 2));
        view.render_ref(Rect::new(0, 0, 2, 2), &mut buf, &theme);
    }

    #[test]
    fn renders_with_highlighter() {
        let diff = "\
diff --git a/main.rs b/main.rs
--- a/main.rs
+++ b/main.rs
@@ -1 +1 @@
 fn main() {}
";
        let mut view = DiffView::new();
        view.set_diff(diff);
        view.set_highlighter(Some(Arc::new(NoHighlight)));
        let theme = Theme::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 50, 3));
        view.render_ref(Rect::new(0, 0, 50, 3), &mut buf, &theme);
    }

    #[test]
    fn applies_highlighter_background_color() {
        struct BgHighlighter;

        impl CodeHighlighter for BgHighlighter {
            fn highlight_lines(
                &self,
                _language: Option<&str>,
                lines: &[&str],
            ) -> Vec<Vec<Span<'static>>> {
                lines
                    .iter()
                    .map(|l| vec![Span::raw((*l).to_string())])
                    .collect()
            }

            fn background_color(&self) -> Option<Color> {
                Some(Color::Blue)
            }
        }

        let diff = "\
diff --git a/a.txt b/a.txt
--- a/a.txt
+++ b/a.txt
@@ -1,1 +1,1 @@
 a
";
        let mut view = DiffView::with_options(DiffViewOptions {
            show_scrollbar: false,
            ..Default::default()
        });
        view.set_highlighter(Some(Arc::new(BgHighlighter)));
        view.set_diff(diff);

        let theme = Theme::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 50, 2));
        view.render_ref(Rect::new(0, 0, 50, 2), &mut buf, &theme);

        assert_eq!(
            buf.cell((49, 0)).expect("cell exists").style().bg,
            Some(Color::Blue)
        );
    }

    #[test]
    fn caches_visible_highlighting_across_renders() {
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

        let diff = "\
diff --git a/main.rs b/main.rs
--- a/main.rs
+++ b/main.rs
@@ -1 +1 @@
 fn main() {}
";

        let highlighter = Arc::new(CountingHighlighter::default());
        let mut view = DiffView::new();
        view.set_highlighter(Some(highlighter.clone()));
        view.set_diff(diff);

        let theme = Theme::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 50, 6));
        view.render_ref(Rect::new(0, 0, 50, 6), &mut buf, &theme);
        view.render_ref(Rect::new(0, 0, 50, 6), &mut buf, &theme);

        assert_eq!(highlighter.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn transcript_lines_cache_highlighting() {
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

        let diff = "\
diff --git a/main.rs b/main.rs
--- a/main.rs
+++ b/main.rs
@@ -1 +1 @@
 fn main() {}
";

        let highlighter = Arc::new(CountingHighlighter::default());
        let mut view = DiffView::new();
        view.set_highlighter(Some(highlighter.clone()));
        view.set_diff(diff);

        let theme = Theme::default();
        let _ = view.lines_for_transcript(&theme);
        let _ = view.lines_for_transcript(&theme);

        assert_eq!(highlighter.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn mouse_click_selects_single_cell() {
        let diff = "\
diff --git a/a.txt b/a.txt
@@ -1,1 +1,1 @@
-a
+b
";

        let mut view = DiffView::with_options(DiffViewOptions {
            show_line_numbers: false,
            show_scrollbar: false,
            ..Default::default()
        });
        view.set_diff(diff);

        let area = Rect::new(0, 0, 20, 5);
        let modifiers = crate::input::KeyModifiers::none();

        assert!(view.handle_mouse_event(
            area,
            MouseEvent {
                x: 2,
                y: 3,
                kind: MouseEventKind::Down(MouseButton::Left),
                modifiers,
            }
        ));
        assert!(view.handle_mouse_event(
            area,
            MouseEvent {
                x: 2,
                y: 3,
                kind: MouseEventKind::Up(MouseButton::Left),
                modifiers,
            }
        ));

        assert_eq!(view.selected_text().as_deref(), Some("b"));
    }
}
