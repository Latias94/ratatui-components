use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::Span;
use ratatui::text::Text;
use unicode_width::UnicodeWidthStr;

use crate::input::InputEvent;
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
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::mpsc;
use std::thread;

#[derive(Clone, Debug)]
pub struct CodeViewOptions {
    pub show_line_numbers: bool,
    pub show_scrollbar: bool,
    pub scroll: ScrollBindings,
    pub enable_selection: bool,
    pub selection: SelectionBindings,
    pub async_highlighting: bool,
    pub max_sync_highlight_lines: usize,
}

impl Default for CodeViewOptions {
    fn default() -> Self {
        Self {
            show_line_numbers: true,
            show_scrollbar: true,
            scroll: ScrollBindings::default(),
            enable_selection: true,
            selection: SelectionBindings::default(),
            async_highlighting: true,
            max_sync_highlight_lines: 200,
        }
    }
}

#[derive(Clone, Debug)]
struct VisibleHighlightCache {
    start: usize,
    end: usize,
    hash: u64,
    spans: std::sync::Arc<std::collections::HashMap<usize, Vec<Span<'static>>>>,
}

#[derive(Clone, Debug)]
struct FullHighlightCache {
    hash: u64,
    spans: std::sync::Arc<Vec<Vec<Span<'static>>>>,
}

struct HighlightRequest {
    hash: u64,
    language: Option<String>,
    lines: std::sync::Arc<Vec<String>>,
}

struct HighlightResult {
    hash: u64,
    highlighted: std::sync::Arc<Vec<Vec<Span<'static>>>>,
}

struct HighlightWorker {
    req_tx: mpsc::Sender<HighlightRequest>,
    res_rx: mpsc::Receiver<HighlightResult>,
}

#[derive(Default)]
pub struct CodeView {
    lines: std::sync::Arc<Vec<String>>,
    max_content_width: u16,
    pub state: ViewportState,
    options: CodeViewOptions,
    highlighter: Option<std::sync::Arc<dyn CodeHighlighter + Send + Sync>>,
    language: Option<String>,
    full_inputs_hash: u64,
    visible_highlight_cache: Option<VisibleHighlightCache>,
    full_highlight_cache: Option<FullHighlightCache>,
    full_highlight_pending: Option<u64>,
    highlight_worker: Option<HighlightWorker>,
    highlight_scratch: String,
    selection_anchor: Option<(usize, u32)>,
    selection: Option<((usize, u32), (usize, u32))>,
}

impl CodeView {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_options(options: CodeViewOptions) -> Self {
        Self {
            options,
            ..Default::default()
        }
    }

    pub fn options(&self) -> &CodeViewOptions {
        &self.options
    }

    pub fn set_options(&mut self, options: CodeViewOptions) {
        self.options = options;
    }

    pub fn handle_event(&mut self, event: InputEvent) -> bool {
        !matches!(self.handle_event_action(event), SelectionAction::None)
    }

    pub fn handle_event_action(&mut self, event: InputEvent) -> SelectionAction {
        match event {
            InputEvent::Paste(_) => SelectionAction::None,
            InputEvent::Mouse(_) => SelectionAction::None,
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

    pub fn handle_event_in_area(&mut self, area: Rect, event: InputEvent) -> bool {
        match event {
            InputEvent::Key(_) => self.handle_event(event),
            InputEvent::Paste(_) => false,
            InputEvent::Mouse(m) => self.handle_mouse_event(area, m),
        }
    }

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

    pub fn handle_mouse_event(&mut self, area: Rect, event: MouseEvent) -> bool {
        if !self.options.enable_selection {
            return false;
        }
        if area.width == 0 || area.height == 0 {
            return false;
        }

        let content_area = if self.options.show_scrollbar && area.width >= 2 {
            Rect::new(area.x, area.y, area.width - 1, area.height)
        } else {
            area
        };

        let gutter_w = if self.options.show_line_numbers {
            (digits(self.lines.len()).saturating_add(1) as u16).min(content_area.width)
        } else {
            0
        };

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

        if event.x < content_area.x + gutter_w
            || event.x >= content_area.x + content_area.width
            || event.y < content_area.y
            || event.y >= content_area.y + content_area.height
        {
            return false;
        }

        let rel_x = (event.x - (content_area.x + gutter_w)) as u32;
        let rel_y = (event.y - content_area.y) as u32;
        let line = self
            .state
            .y
            .saturating_add(rel_y)
            .min(self.lines.len().saturating_sub(1) as u32) as usize;
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
        let ((start_line, start_col), (end_line, end_col)) = normalize_sel((l0, c0), (l1, c1));

        let mut out = String::new();
        for line_idx in start_line..=end_line {
            let line = self.lines.get(line_idx)?;
            let (from, to) = if start_line == end_line {
                (start_col, end_col)
            } else if line_idx == start_line {
                (start_col, u32::MAX)
            } else if line_idx == end_line {
                (0, end_col)
            } else {
                (0, u32::MAX)
            };

            if line_idx > start_line {
                out.push('\n');
            }

            if let Some((bs, be)) = render::byte_range_for_cols(line, from, to) {
                out.push_str(&line[bs..be]);
            }
        }
        Some(out)
    }

    pub fn set_highlighter(
        &mut self,
        highlighter: Option<std::sync::Arc<dyn CodeHighlighter + Send + Sync>>,
    ) {
        self.highlighter = highlighter;
        self.invalidate_highlighting();
    }

    pub fn set_language(&mut self, language: Option<impl Into<String>>) {
        self.language = language.map(Into::into);
        self.recompute_full_inputs_hash();
        self.invalidate_highlighting();
    }

    pub fn set_code(&mut self, code: &str) {
        let mut lines = code.lines().map(|l| normalize_tabs(l)).collect::<Vec<_>>();
        if code.ends_with('\n') {
            lines.push(String::new());
        }
        self.set_lines(lines);
    }

    pub fn set_lines(&mut self, mut lines: Vec<String>) {
        for l in &mut lines {
            if l.contains('\t') {
                *l = normalize_tabs(l);
            }
        }

        self.max_content_width = lines
            .iter()
            .map(|l| UnicodeWidthStr::width(l.as_str()) as u16)
            .max()
            .unwrap_or(0);
        let h = lines.len() as u32;
        self.lines = std::sync::Arc::new(lines);
        self.recompute_full_inputs_hash();
        self.invalidate_highlighting();
        self.state
            .set_content(self.max_content_width as u32, h.max(0));
    }

    pub fn scroll_y_by(&mut self, delta: i32) {
        self.state.scroll_y_by(delta);
    }

    pub fn scroll_x_by(&mut self, delta: i32) {
        self.state.scroll_x_by(delta);
    }

    pub fn set_viewport(&mut self, area: Rect) {
        let content_area = if self.options.show_scrollbar && area.width >= 2 {
            Rect::new(area.x, area.y, area.width - 1, area.height)
        } else {
            area
        };

        let gutter_w = if self.options.show_line_numbers {
            let digits = digits(self.lines.len());
            digits.saturating_add(1) as u16
        } else {
            0
        };
        let viewport_w = content_area.width.saturating_sub(gutter_w);
        self.state.set_viewport(viewport_w, content_area.height);
    }

    pub fn render_ref(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        self.poll_highlight_results();

        self.set_viewport(area);

        let (content_area, scrollbar_x) = if self.options.show_scrollbar && area.width >= 2 {
            (
                Rect::new(area.x, area.y, area.width - 1, area.height),
                Some(area.x + area.width - 1),
            )
        } else {
            (area, None)
        };

        let gutter_w = if self.options.show_line_numbers {
            (digits(self.lines.len()).saturating_add(1) as u16).min(content_area.width)
        } else {
            0
        };
        let content_w = content_area.width.saturating_sub(gutter_w);

        let (base_style, gutter_style) = if let Some(bg) = self.highlighter.as_ref().and_then(|h| h.background_color())
        {
            (theme.text_primary.bg(bg), theme.text_muted.bg(bg))
        } else {
            (theme.text_primary, theme.text_muted)
        };

        let start = self.state.y as usize;
        let end = (start + content_area.height as usize).min(self.lines.len());

        let highlighted_full = if self.highlighter.is_some() && content_w > 0 {
            self.ensure_full_highlighting();
            self.full_highlight_cache.as_ref().map(|c| c.spans.clone())
        } else {
            None
        };

        let highlighted_visible = if highlighted_full.is_none()
            && self.highlighter.is_some()
            && content_w > 0
            && end.saturating_sub(start) <= self.options.max_sync_highlight_lines
        {
            Some(self.highlight_visible_cached(start, end))
        } else {
            None
        };

        for row in 0..content_area.height {
            let y = content_area.y + row;
            let idx = (self.state.y as usize).saturating_add(row as usize);

            buf.set_style(
                Rect::new(content_area.x, y, content_area.width, 1),
                base_style,
            );

            if self.options.show_line_numbers && gutter_w > 0 {
                let lineno = if idx < self.lines.len() {
                    format!(
                        "{:>width$} ",
                        idx + 1,
                        width = (gutter_w as usize).saturating_sub(1)
                    )
                } else {
                    " ".repeat(gutter_w as usize)
                };
                buf.set_stringn(
                    content_area.x,
                    y,
                    lineno,
                    content_area.width as usize,
                    gutter_style,
                );
            }

            if content_w == 0 {
                continue;
            }

            let Some(line) = self.lines.get(idx) else {
                continue;
            };

            let mut spans = if let Some(all) = highlighted_full.as_ref()
                && let Some(spans) = all.get(idx)
            {
                spans.clone()
            } else if let Some(m) = highlighted_visible.as_ref()
                && let Some(spans) = m.get(&idx)
            {
                spans.clone()
            } else {
                vec![Span::styled(line.clone(), theme.text_primary)]
            };

            if self.options.enable_selection
                && let Some(((l0, c0), (l1, c1))) = self.selection
            {
                let ((start_line, start_col), (end_line, end_col)) =
                    normalize_sel((l0, c0), (l1, c1));
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
                    if let Some((bs, be)) = render::byte_range_for_cols(line, from, to) {
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
                base_style,
            );
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
        let mut out = Vec::with_capacity(self.lines.len());
        for (idx, line) in self.lines.iter().enumerate() {
            if self.options.show_line_numbers {
                out.push(ratatui::text::Line::from(vec![
                    Span::styled(format!("{:>4} ", idx + 1), theme.text_muted),
                    Span::styled(line.clone(), theme.text_primary),
                ]));
            } else {
                out.push(ratatui::text::Line::styled(
                    line.clone(),
                    theme.text_primary,
                ));
            }
        }
        Text::from(out)
    }

    fn highlight_visible_cached(
        &mut self,
        start: usize,
        end: usize,
    ) -> std::sync::Arc<std::collections::HashMap<usize, Vec<Span<'static>>>> {
        let hash = compute_hash(&self.language, self.lines.as_ref(), start, end);
        if let Some(cache) = self.visible_highlight_cache.as_ref()
            && cache.start == start
            && cache.end == end
            && cache.hash == hash
        {
            return cache.spans.clone();
        }

        let Some(highlighter) = self.highlighter.as_ref() else {
            return std::sync::Arc::new(std::collections::HashMap::new());
        };

        self.highlight_scratch.clear();
        for (i, line) in self.lines[start..end].iter().enumerate() {
            if i > 0 {
                self.highlight_scratch.push('\n');
            }
            self.highlight_scratch.push_str(line);
        }
        let highlighted = highlighter.highlight_text(self.language.as_deref(), &self.highlight_scratch);
        let mut map = std::collections::HashMap::new();
        for (i, spans) in highlighted.into_iter().enumerate() {
            map.insert(start + i, spans);
        }

        let spans = std::sync::Arc::new(map);
        self.visible_highlight_cache = Some(VisibleHighlightCache {
            start,
            end,
            hash,
            spans: spans.clone(),
        });
        spans
    }

    fn invalidate_highlighting(&mut self) {
        self.visible_highlight_cache = None;
        self.full_highlight_cache = None;
        self.full_highlight_pending = None;
        self.highlight_worker = None;
    }

    fn recompute_full_inputs_hash(&mut self) {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.language.hash(&mut h);
        for l in self.lines.iter() {
            l.hash(&mut h);
        }
        self.full_inputs_hash = h.finish();
    }

    fn poll_highlight_results(&mut self) {
        let Some(worker) = self.highlight_worker.as_ref() else {
            return;
        };
        while let Ok(res) = worker.res_rx.try_recv() {
            if res.hash != self.full_inputs_hash {
                continue;
            }
            self.full_highlight_cache = Some(FullHighlightCache {
                hash: res.hash,
                spans: res.highlighted,
            });
            self.full_highlight_pending = None;
        }
    }

    fn ensure_highlight_worker(&mut self) {
        if self.highlight_worker.is_some() {
            return;
        }
        let Some(hi) = self.highlighter.clone() else {
            return;
        };

        let (req_tx, req_rx) = mpsc::channel::<HighlightRequest>();
        let (res_tx, res_rx) = mpsc::channel::<HighlightResult>();

        thread::spawn(move || {
            while let Ok(req) = req_rx.recv() {
                let refs: Vec<&str> = req.lines.iter().map(|s| s.as_str()).collect();
                let highlighted = hi.highlight_lines(req.language.as_deref(), &refs);
                let res = HighlightResult {
                    hash: req.hash,
                    highlighted: std::sync::Arc::new(highlighted),
                };
                if res_tx.send(res).is_err() {
                    break;
                }
            }
        });

        self.highlight_worker = Some(HighlightWorker { req_tx, res_rx });
    }

    fn ensure_full_highlighting(&mut self) {
        if !self.options.async_highlighting {
            return;
        }
        let Some(_) = self.highlighter.as_ref() else {
            return;
        };
        if self.lines.is_empty() {
            return;
        }

        let hash = self.full_inputs_hash;
        if let Some(cache) = self.full_highlight_cache.as_ref()
            && cache.hash == hash
        {
            return;
        }

        if self.full_highlight_pending == Some(hash) {
            return;
        }

        self.ensure_highlight_worker();
        let Some(worker) = self.highlight_worker.as_ref() else {
            return;
        };

        let req = HighlightRequest {
            hash,
            language: self.language.clone(),
            lines: self.lines.clone(),
        };

        if worker.req_tx.send(req).is_ok() {
            self.full_highlight_pending = Some(hash);
        } else {
            self.highlight_worker = None;
        }
    }
}

impl Clone for CodeView {
    fn clone(&self) -> Self {
        Self {
            lines: self.lines.clone(),
            max_content_width: self.max_content_width,
            state: self.state,
            options: self.options.clone(),
            highlighter: self.highlighter.clone(),
            language: self.language.clone(),
            full_inputs_hash: self.full_inputs_hash,
            visible_highlight_cache: None,
            full_highlight_cache: None,
            full_highlight_pending: None,
            highlight_worker: None,
            highlight_scratch: String::new(),
            selection_anchor: self.selection_anchor,
            selection: self.selection,
        }
    }
}

fn compute_hash(language: &Option<String>, lines: &[String], start: usize, end: usize) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    language.hash(&mut h);
    start.hash(&mut h);
    end.hash(&mut h);
    for l in &lines[start..end] {
        l.hash(&mut h);
    }
    h.finish()
}

fn digits(mut n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    let mut d = 0;
    while n > 0 {
        n /= 10;
        d += 1;
    }
    d
}

fn normalize_tabs(s: &str) -> String {
    if s.contains('\t') {
        s.replace('\t', "    ")
    } else {
        s.to_string()
    }
}

fn normalize_sel(a: (usize, u32), b: (usize, u32)) -> ((usize, u32), (usize, u32)) {
    if a.0 < b.0 || (a.0 == b.0 && a.1 <= b.1) {
        (a, b)
    } else {
        (b, a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::style::Color;
    use ratatui::style::Style;
    use std::sync::Arc;

    #[test]
    fn code_view_renders_without_panic() {
        let mut v = CodeView::new();
        v.set_code("fn main() {\n\tprintln!(\"hi\");\n}\n");
        let theme = Theme::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 3));
        v.render_ref(Rect::new(0, 0, 20, 3), &mut buf, &theme);
    }

    #[test]
    fn code_view_applies_highlighter_background_to_trailing_cells() {
        struct BgHighlighter;

        impl CodeHighlighter for BgHighlighter {
            fn highlight_lines(
                &self,
                _language: Option<&str>,
                lines: &[&str],
            ) -> Vec<Vec<Span<'static>>> {
                lines
                    .iter()
                    .map(|l| vec![Span::styled((*l).to_string(), Style::default())])
                    .collect()
            }

            fn background_color(&self) -> Option<Color> {
                Some(Color::Blue)
            }
        }

        let mut v = CodeView::with_options(CodeViewOptions {
            show_line_numbers: false,
            show_scrollbar: false,
            ..Default::default()
        });
        v.set_code("hi");
        v.set_highlighter(Some(Arc::new(BgHighlighter)));

        let theme = Theme::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
        v.render_ref(Rect::new(0, 0, 10, 1), &mut buf, &theme);

        assert_eq!(
            buf.cell((9, 0)).expect("cell exists").style().bg,
            Some(Color::Blue)
        );
    }
}
