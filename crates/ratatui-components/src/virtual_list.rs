use crate::input::InputEvent;
use crate::input::KeyCode;
use crate::input::KeyEvent;
use crate::render;
use crate::theme::Theme;
use crate::viewport::ViewportState;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::style::Style;
use std::collections::BTreeSet;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use virtualizer::Align;
use virtualizer::ItemKey;
use virtualizer::VirtualItem;
use virtualizer::Virtualizer;
use virtualizer::VirtualizerOptions;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtualListAction {
    None,
    Redraw,
    Activated(usize),
    SelectionChanged,
}

#[derive(Clone, Debug)]
pub struct VirtualListViewOptions {
    pub show_scrollbar: bool,
    pub overscan: usize,
    pub gap: u32,
    pub padding_top: u32,
    pub padding_bottom: u32,
    pub scroll_padding_top: u32,
    pub scroll_padding_bottom: u32,
    pub style: Style,
    pub scrollbar_style: Style,
    pub cursor_style: Style,
    pub selected_style: Style,
    pub selection_follows_cursor: bool,
    pub multi_select: bool,
}

impl Default for VirtualListViewOptions {
    fn default() -> Self {
        Self {
            show_scrollbar: true,
            overscan: 2,
            gap: 0,
            padding_top: 0,
            padding_bottom: 0,
            scroll_padding_top: 0,
            scroll_padding_bottom: 0,
            style: Style::default(),
            scrollbar_style: Style::default(),
            cursor_style: Style::default().add_modifier(Modifier::REVERSED),
            selected_style: Style::default().add_modifier(Modifier::BOLD),
            selection_follows_cursor: true,
            multi_select: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct VirtualListItemContext {
    pub index: usize,
    pub key: ItemKey,
    pub item: VirtualItem,
    /// Offset into the item due to top clipping (in scroll-axis units).
    pub clip_top: u32,
    pub is_cursor: bool,
    pub is_selected: bool,
    pub x_scroll: u32,
}

pub struct VirtualListView {
    pub viewport: ViewportState,
    options: VirtualListViewOptions,
    virtualizer: Virtualizer,

    cursor: Option<usize>,
    selection: BTreeSet<usize>,
    selection_anchor: Option<usize>,

    content_w: Option<u32>,
    cached_width: Option<u16>,
    width_cell: Arc<AtomicU32>,
    estimator: Arc<dyn Fn(usize, u16) -> u32 + Send + Sync>,
}

impl Default for VirtualListView {
    fn default() -> Self {
        let width_cell = Arc::new(AtomicU32::new(0));
        let estimator: Arc<dyn Fn(usize, u16) -> u32 + Send + Sync> = Arc::new(|_, _| 1);
        let v = Self::make_virtualizer(0, width_cell.clone(), estimator.clone(), 2);

        Self {
            viewport: ViewportState::default(),
            options: VirtualListViewOptions::default(),
            virtualizer: v,
            cursor: None,
            selection: BTreeSet::new(),
            selection_anchor: None,
            content_w: None,
            cached_width: None,
            width_cell,
            estimator,
        }
    }
}

impl VirtualListView {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_options(options: VirtualListViewOptions) -> Self {
        let mut v = Self::default();
        v.set_options(options);
        v
    }

    pub fn options(&self) -> &VirtualListViewOptions {
        &self.options
    }

    pub fn set_options(&mut self, options: VirtualListViewOptions) {
        self.options = options;
        self.virtualizer.set_overscan(self.options.overscan);
        self.virtualizer
            .set_padding(self.options.padding_top, self.options.padding_bottom);
        self.virtualizer.set_scroll_padding(
            self.options.scroll_padding_top,
            self.options.scroll_padding_bottom,
        );
        self.virtualizer.set_gap(self.options.gap);
    }

    pub fn cursor(&self) -> Option<usize> {
        self.cursor
    }

    pub fn selected(&self) -> &BTreeSet<usize> {
        &self.selection
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.selection_anchor = None;
    }

    pub fn set_content_width(&mut self, w: Option<u32>) {
        self.content_w = w;
        self.viewport.clamp();
    }

    pub fn set_fixed_item_size(&mut self, size: u32) {
        self.set_estimator(move |_, _| size);
    }

    pub fn set_estimator(&mut self, f: impl Fn(usize, u16) -> u32 + Send + Sync + 'static) {
        self.estimator = Arc::new(f);
        self.virtualizer = Self::make_virtualizer(
            self.virtualizer.count(),
            self.width_cell.clone(),
            self.estimator.clone(),
            self.options.overscan,
        );
        self.cached_width = None;
        self.viewport.clamp();
    }

    pub fn set_get_item_key(&mut self, f: impl Fn(usize) -> ItemKey + Send + Sync + 'static) {
        self.virtualizer.set_get_item_key(f);
    }

    pub fn set_range_extractor(
        &mut self,
        f: Option<impl Fn(virtualizer::VirtualRange) -> Vec<usize> + Send + Sync + 'static>,
    ) {
        self.virtualizer.set_range_extractor(f);
    }

    pub fn set_cursor(&mut self, cursor: Option<usize>, count: usize) {
        self.cursor = clamp_cursor(cursor, count);
        if !self.options.multi_select && self.options.selection_follows_cursor {
            self.selection.clear();
            if let Some(c) = self.cursor {
                self.selection.insert(c);
                self.selection_anchor = Some(c);
            } else {
                self.selection_anchor = None;
            }
        }
        self.ensure_cursor_visible(count);
    }

    pub fn ensure_cursor_visible(&mut self, count: usize) {
        if count == 0 {
            self.cursor = None;
            self.viewport.set_content(self.content_w.unwrap_or(0), 0);
            self.viewport.y = 0;
            return;
        }
        self.sync_virtualizer(count);
        if let Some(cursor) = self.cursor {
            self.virtualizer.scroll_to_index(cursor, Align::Auto);
            self.viewport.y = self.virtualizer.scroll_offset().min(u32::MAX as u64) as u32;
        }
        self.viewport.clamp();
    }

    pub fn handle_event(&mut self, event: InputEvent, count: usize) -> VirtualListAction {
        match event {
            InputEvent::Paste(_) => VirtualListAction::None,
            InputEvent::Key(key) => self.handle_key(key, count),
        }
    }

    pub fn render<F>(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        count: usize,
        mut render_item: F,
    ) where
        F: FnMut(Rect, VirtualListItemContext, &mut Buffer, &Theme) -> Option<u32>,
    {
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

        self.viewport.set_viewport(content_area.width, content_area.height);
        self.width_cell
            .store(content_area.width as u32, Ordering::Relaxed);
        if self.cached_width != Some(content_area.width) {
            self.cached_width = Some(content_area.width);
            let closure = Self::estimate_closure(self.width_cell.clone(), self.estimator.clone());
            self.virtualizer.set_estimate_size(closure);
        }

        self.sync_virtualizer(count);

        let content_w = self
            .content_w
            .unwrap_or_else(|| content_area.width as u32)
            .max(content_area.width as u32);
        self.viewport
            .set_content(content_w, self.total_size_u32());
        self.viewport.y = self.virtualizer.scroll_offset().min(u32::MAX as u64) as u32;

        let base_style = if self.options.style == Style::default() {
            theme.text_primary
        } else {
            self.options.style
        };
        buf.set_style(content_area, base_style);

        let cursor_style = self.options.cursor_style.patch(theme.accent);
        let selected_style = self.options.selected_style.patch(theme.accent);

        let items = self.virtualizer.get_virtual_items();
        let scroll = self.virtualizer.scroll_offset();

        let mut measurements: Vec<(usize, u32)> = Vec::new();
        for item in items {
            let rel_start = item.start as i64 - scroll as i64;
            let clip_top = (-rel_start).max(0) as u32;
            let visible_start = rel_start.max(0) as u16;
            let remaining_h = content_area.height.saturating_sub(visible_start);
            if remaining_h == 0 {
                continue;
            }

            let visible_h_u32 = item.size.saturating_sub(clip_top);
            if visible_h_u32 == 0 {
                continue;
            }
            let visible_h = (visible_h_u32.min(remaining_h as u32)).min(u16::MAX as u32) as u16;
            if visible_h == 0 {
                continue;
            }

            let item_area = Rect::new(
                content_area.x,
                content_area.y + visible_start,
                content_area.width,
                visible_h,
            );

            let idx = item.index;
            let is_cursor = self.cursor == Some(idx);
            let is_selected = self.selection.contains(&idx);
            let style = if is_cursor {
                cursor_style
            } else if is_selected {
                selected_style
            } else {
                base_style
            };
            buf.set_style(item_area, style);

            let ctx = VirtualListItemContext {
                index: idx,
                key: self.virtualizer.key_for(idx),
                item,
                clip_top,
                is_cursor,
                is_selected,
                x_scroll: self.viewport.x,
            };
            if let Some(measured) = render_item(item_area, ctx, buf, theme) {
                measurements.push((idx, measured));
            }
        }

        for (idx, measured) in measurements {
            self.virtualizer.measure(idx, measured);
        }
        self.viewport
            .set_content(content_w, self.total_size_u32());
        self.viewport.y = self.virtualizer.scroll_offset().min(u32::MAX as u64) as u32;
        self.viewport.clamp();

        if let Some(sb_x) = scrollbar_x {
            render::render_scrollbar(
                Rect::new(sb_x, area.y, 1, area.height),
                buf,
                &self.viewport,
                self.options.scrollbar_style,
            );
        }
    }

    fn handle_key(&mut self, key: KeyEvent, count: usize) -> VirtualListAction {
        if count == 0 {
            self.cursor = None;
            self.clear_selection();
            self.viewport.y = 0;
            self.viewport.clamp();
            return VirtualListAction::None;
        }

        self.sync_virtualizer(count);

        if key.modifiers.ctrl && !key.modifiers.alt {
            if matches!(key.code, KeyCode::Char('d')) {
                let delta = self.viewport.viewport_h.saturating_sub(1) as i32;
                self.viewport.scroll_y_by(delta);
                self.virtualizer.set_scroll_offset(self.viewport.y as u64);
                self.viewport.y = self.virtualizer.scroll_offset().min(u32::MAX as u64) as u32;
                self.set_cursor_from_scroll(count);
                return VirtualListAction::Redraw;
            }
            if matches!(key.code, KeyCode::Char('u')) {
                let delta = -(self.viewport.viewport_h.saturating_sub(1) as i32);
                self.viewport.scroll_y_by(delta);
                self.virtualizer.set_scroll_offset(self.viewport.y as u64);
                self.viewport.y = self.virtualizer.scroll_offset().min(u32::MAX as u64) as u32;
                self.set_cursor_from_scroll(count);
                return VirtualListAction::Redraw;
            }
        }

        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                let moved = self.move_cursor_by(1, count, key.modifiers.shift);
                if moved {
                    VirtualListAction::Redraw
                } else {
                    VirtualListAction::None
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let moved = self.move_cursor_by(-1, count, key.modifiers.shift);
                if moved {
                    VirtualListAction::Redraw
                } else {
                    VirtualListAction::None
                }
            }
            KeyCode::PageDown => {
                self.viewport.page_down();
                self.virtualizer.set_scroll_offset(self.viewport.y as u64);
                self.viewport.y = self.virtualizer.scroll_offset().min(u32::MAX as u64) as u32;
                self.set_cursor_from_scroll(count);
                VirtualListAction::Redraw
            }
            KeyCode::PageUp => {
                self.viewport.page_up();
                self.virtualizer.set_scroll_offset(self.viewport.y as u64);
                self.viewport.y = self.virtualizer.scroll_offset().min(u32::MAX as u64) as u32;
                self.set_cursor_from_scroll(count);
                VirtualListAction::Redraw
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.cursor = Some(0);
                self.selection_anchor = Some(0);
                self.ensure_cursor_visible(count);
                if !self.options.multi_select && self.options.selection_follows_cursor {
                    self.selection.clear();
                    self.selection.insert(0);
                }
                VirtualListAction::Redraw
            }
            KeyCode::End | KeyCode::Char('G') => {
                let last = count.saturating_sub(1);
                self.cursor = Some(last);
                self.selection_anchor = Some(last);
                self.ensure_cursor_visible(count);
                if !self.options.multi_select && self.options.selection_follows_cursor {
                    self.selection.clear();
                    self.selection.insert(last);
                }
                VirtualListAction::Redraw
            }
            KeyCode::Enter => self
                .cursor
                .map(VirtualListAction::Activated)
                .unwrap_or(VirtualListAction::None),
            KeyCode::Char(' ') => {
                if let Some(cursor) = self.cursor {
                    let before = self.selection.contains(&cursor);
                    if self.options.multi_select {
                        if before {
                            self.selection.remove(&cursor);
                        } else {
                            self.selection.insert(cursor);
                            self.selection_anchor = Some(cursor);
                        }
                    } else {
                        self.selection.clear();
                        self.selection.insert(cursor);
                        self.selection_anchor = Some(cursor);
                    }
                    let after = self.selection.contains(&cursor);
                    if before != after {
                        VirtualListAction::SelectionChanged
                    } else {
                        VirtualListAction::Redraw
                    }
                } else {
                    VirtualListAction::None
                }
            }
            _ => VirtualListAction::None,
        }
    }

    fn move_cursor_by(&mut self, delta: i32, count: usize, shift: bool) -> bool {
        let cur = self.cursor.unwrap_or(0);
        let next = (cur as i64 + delta as i64).clamp(0, count.saturating_sub(1) as i64) as usize;
        if Some(next) == self.cursor {
            return false;
        }
        self.cursor = Some(next);

        if self.options.multi_select && shift {
            let anchor = self.selection_anchor.unwrap_or(cur);
            self.selection_anchor = Some(anchor);
            let (a, b) = if anchor <= next {
                (anchor, next)
            } else {
                (next, anchor)
            };
            for i in a..=b {
                self.selection.insert(i);
            }
        } else if !self.options.multi_select && self.options.selection_follows_cursor {
            self.selection.clear();
            self.selection.insert(next);
            self.selection_anchor = Some(next);
        } else {
            self.selection_anchor = Some(next);
        }

        self.ensure_cursor_visible(count);
        true
    }

    fn set_cursor_from_scroll(&mut self, count: usize) {
        self.sync_virtualizer(count);
        if let Some(i) = self.virtualizer.index_at_offset(self.virtualizer.scroll_offset()) {
            self.cursor = Some(i);
            if !self.options.multi_select && self.options.selection_follows_cursor {
                self.selection.clear();
                self.selection.insert(i);
                self.selection_anchor = Some(i);
            }
        }
    }

    fn sync_virtualizer(&mut self, count: usize) {
        self.virtualizer.set_count(count);
        self.virtualizer
            .set_viewport_size(self.viewport.viewport_h as u32);
        self.virtualizer.set_overscan(self.options.overscan);
        self.virtualizer
            .set_padding(self.options.padding_top, self.options.padding_bottom);
        self.virtualizer.set_scroll_padding(
            self.options.scroll_padding_top,
            self.options.scroll_padding_bottom,
        );
        self.virtualizer.set_gap(self.options.gap);
        self.virtualizer.set_scroll_offset(self.viewport.y as u64);
        self.viewport.y = self.virtualizer.scroll_offset().min(u32::MAX as u64) as u32;

        let content_w = self
            .content_w
            .unwrap_or_else(|| self.viewport.viewport_w as u32)
            .max(self.viewport.viewport_w as u32);
        self.viewport
            .set_content(content_w, self.total_size_u32());
    }

    fn total_size_u32(&self) -> u32 {
        self.virtualizer.get_total_size().min(u32::MAX as u64) as u32
    }

    fn make_virtualizer(
        count: usize,
        width_cell: Arc<AtomicU32>,
        estimator: Arc<dyn Fn(usize, u16) -> u32 + Send + Sync>,
        overscan: usize,
    ) -> Virtualizer {
        let mut opts = VirtualizerOptions::new(count, Self::estimate_closure(width_cell, estimator));
        opts.overscan = overscan;
        Virtualizer::new(opts)
    }

    fn estimate_closure(
        width_cell: Arc<AtomicU32>,
        estimator: Arc<dyn Fn(usize, u16) -> u32 + Send + Sync>,
    ) -> impl Fn(usize) -> u32 + Send + Sync + 'static {
        move |idx| {
            let w = width_cell.load(Ordering::Relaxed).min(u16::MAX as u32) as u16;
            estimator(idx, w)
        }
    }
}

fn clamp_cursor(cursor: Option<usize>, count: usize) -> Option<usize> {
    match cursor {
        None => None,
        Some(_) if count == 0 => None,
        Some(i) => Some(i.min(count - 1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code)
    }

    fn key_shift(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code).with_modifiers(KeyModifiers {
            shift: true,
            ctrl: false,
            alt: false,
        })
    }

    #[test]
    fn cursor_moves_and_stays_visible() {
        let mut v = VirtualListView::new();
        v.set_fixed_item_size(1);
        v.viewport.set_viewport(10, 3);
        v.set_cursor(Some(0), 100);
        assert_eq!(v.cursor(), Some(0));
        assert_eq!(v.viewport.y, 0);

        v.handle_event(InputEvent::Key(key(KeyCode::Down)), 100);
        v.handle_event(InputEvent::Key(key(KeyCode::Down)), 100);
        v.handle_event(InputEvent::Key(key(KeyCode::Down)), 100);
        assert_eq!(v.cursor(), Some(3));
        assert_eq!(v.viewport.y, 1);
    }

    #[test]
    fn shift_extends_selection_in_multi_mode() {
        let mut v = VirtualListView::with_options(VirtualListViewOptions {
            multi_select: true,
            selection_follows_cursor: false,
            ..Default::default()
        });
        v.set_fixed_item_size(1);
        v.viewport.set_viewport(10, 5);
        v.set_cursor(Some(5), 20);
        v.clear_selection();
        v.selection_anchor = Some(5);

        v.handle_event(InputEvent::Key(key_shift(KeyCode::Down)), 20);
        v.handle_event(InputEvent::Key(key_shift(KeyCode::Down)), 20);
        assert!(v.selected().contains(&5));
        assert!(v.selected().contains(&6));
        assert!(v.selected().contains(&7));
    }
}
