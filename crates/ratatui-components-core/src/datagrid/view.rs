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
use ratatui::text::Span;
use virtualizer::Align;
use virtualizer::Virtualizer;
use virtualizer::VirtualizerOptions;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataGridAction {
    None,
    Redraw,
    Activated(Cell),
    SelectionChanged,
}

/// A grid cell address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cell {
    pub row: usize,
    pub col: usize,
}

/// Selection states supported by [`DataGridView`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selection {
    None,
    Single(Cell),
    Rect { start: Cell, end: Cell },
}

impl Selection {
    pub fn contains(&self, cell: Cell) -> bool {
        match *self {
            Selection::None => false,
            Selection::Single(c) => c == cell,
            Selection::Rect { start, end } => {
                let (r0, r1) = if start.row <= end.row {
                    (start.row, end.row)
                } else {
                    (end.row, start.row)
                };
                let (c0, c1) = if start.col <= end.col {
                    (start.col, end.col)
                } else {
                    (end.col, start.col)
                };
                cell.row >= r0 && cell.row <= r1 && cell.col >= c0 && cell.col <= c1
            }
        }
    }
}

/// Column configuration for [`DataGridView`].
#[derive(Clone, Debug)]
pub struct DataGridColumn {
    pub title: String,
    pub width: u16,
}

impl DataGridColumn {
    pub fn new(title: impl Into<String>, width: u16) -> Self {
        Self {
            title: title.into(),
            width,
        }
    }
}

/// Options for [`DataGridView`].
///
/// This widget virtualizes both rows and columns via the `virtualizer` crate and delegates cell
/// rendering to a user callback.
#[derive(Clone, Debug)]
pub struct DataGridViewOptions {
    pub show_header: bool,
    pub show_scrollbar_y: bool,
    pub overscan_rows: usize,
    pub overscan_cols: usize,
    pub row_height: u32,
    pub col_gap: u32,
    pub style: Style,
    pub header_style: Style,
    pub grid_line_style: Style,
    pub scrollbar_style: Style,
    pub cursor_style: Style,
    pub selected_style: Style,
    pub selection_follows_cursor: bool,
    pub multi_select: bool,
}

impl Default for DataGridViewOptions {
    fn default() -> Self {
        Self {
            show_header: true,
            show_scrollbar_y: true,
            overscan_rows: 2,
            overscan_cols: 2,
            row_height: 1,
            col_gap: 1,
            style: Style::default(),
            header_style: Style::default().add_modifier(Modifier::BOLD),
            grid_line_style: Style::default(),
            scrollbar_style: Style::default(),
            cursor_style: Style::default().add_modifier(Modifier::REVERSED),
            selected_style: Style::default().add_modifier(Modifier::BOLD),
            selection_follows_cursor: true,
            multi_select: false,
        }
    }
}

/// Context passed to the `render_cell` callback in [`DataGridView::render`].
#[derive(Clone, Debug)]
pub struct DataGridCellContext {
    pub cell: Cell,
    pub col_width: u16,
    pub row_start: u64,
    pub row_size: u32,
    pub col_start: u64,
    pub col_size: u32,
    pub clip_left: u32,
    pub clip_top: u32,
    pub is_cursor: bool,
    pub is_selected: bool,
}

/// A virtualized 2D grid view with keyboard navigation and optional selection.
///
/// This widget is designed for large tables (many rows/cols) where rendering all cells would be
/// too expensive.
///
/// The grid itself is UI-agnostic: you drive it from your app loop by calling `handle_event` and
/// `render`.
pub struct DataGridView {
    pub state: ViewportState,
    options: DataGridViewOptions,
    columns: Vec<DataGridColumn>,
    rows: usize,
    cursor: Option<Cell>,
    selection: Selection,
    selection_anchor: Option<Cell>,
    row_v: Virtualizer,
    col_v: Virtualizer,
    row_items: Vec<virtualizer::VirtualItem>,
    col_items: Vec<virtualizer::VirtualItem>,
}

impl Default for DataGridView {
    fn default() -> Self {
        let options = DataGridViewOptions::default();

        let row_height = options.row_height.max(1);
        let overscan_rows = options.overscan_rows;
        let overscan_cols = options.overscan_cols;
        let col_gap = options.col_gap;

        let mut row_opts = VirtualizerOptions::new(0, move |_| row_height);
        row_opts.overscan = overscan_rows;
        let row_v = Virtualizer::new(row_opts);

        let mut col_opts = VirtualizerOptions::new(0, |_| 1);
        col_opts.gap = col_gap;
        col_opts.overscan = overscan_cols;
        let col_v = Virtualizer::new(col_opts);

        Self {
            state: ViewportState::default(),
            options,
            columns: Vec::new(),
            rows: 0,
            cursor: None,
            selection: Selection::None,
            selection_anchor: None,
            row_v,
            col_v,
            row_items: Vec::new(),
            col_items: Vec::new(),
        }
    }
}

#[derive(Clone, Copy)]
struct DataGridBodyStyles {
    base: Style,
    cursor: Style,
    selected: Style,
    grid_line: Style,
}

struct ColSeparatorContext<'a> {
    area: Rect,
    scroll_x: u64,
    col_count: usize,
    buf: &'a mut Buffer,
    style: Style,
}

struct RenderBodyContext<'a> {
    area: Rect,
    buf: &'a mut Buffer,
    row_items: &'a [virtualizer::VirtualItem],
    col_items: &'a [virtualizer::VirtualItem],
    theme: &'a Theme,
}

impl DataGridView {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_options(options: DataGridViewOptions) -> Self {
        let mut v = Self::default();
        v.set_options(options);
        v
    }

    pub fn options(&self) -> &DataGridViewOptions {
        &self.options
    }

    pub fn set_options(&mut self, options: DataGridViewOptions) {
        self.options = options;
        self.rebuild_row_virtualizer();
        self.rebuild_col_virtualizer();
        self.state.clamp();
    }

    pub fn set_row_count(&mut self, rows: usize) {
        self.rows = rows;
        self.rebuild_row_virtualizer();
        self.cursor = clamp_cursor(self.cursor, self.rows, self.columns.len());
        if self.options.selection_follows_cursor {
            self.selection = self
                .cursor
                .map(Selection::Single)
                .unwrap_or(Selection::None);
        }
        self.state.clamp();
    }

    pub fn set_columns(&mut self, columns: Vec<DataGridColumn>) {
        self.columns = columns;
        self.rebuild_col_virtualizer();
        self.cursor = clamp_cursor(self.cursor, self.rows, self.columns.len());
        if self.options.selection_follows_cursor {
            self.selection = self
                .cursor
                .map(Selection::Single)
                .unwrap_or(Selection::None);
        }
        self.state.clamp();
    }

    pub fn row_count(&self) -> usize {
        self.rows
    }

    pub fn columns(&self) -> &[DataGridColumn] {
        &self.columns
    }

    pub fn cursor(&self) -> Option<Cell> {
        self.cursor
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    pub fn set_cursor(&mut self, cursor: Option<Cell>) {
        self.cursor = clamp_cursor(cursor, self.rows, self.columns.len());
        if self.options.selection_follows_cursor {
            self.selection = self
                .cursor
                .map(Selection::Single)
                .unwrap_or(Selection::None);
        }
        self.ensure_cursor_visible();
    }

    pub fn clear_selection(&mut self) {
        self.selection = Selection::None;
        self.selection_anchor = None;
    }

    pub fn handle_event(&mut self, event: InputEvent) -> DataGridAction {
        match event {
            InputEvent::Paste(_) => DataGridAction::None,
            InputEvent::Key(key) => self.handle_key(key),
            InputEvent::Mouse(_) => DataGridAction::None,
        }
    }

    pub fn scroll_y_by(&mut self, delta: i32) {
        self.sync_virtualizers_from_state();
        self.state.scroll_y_by(delta);
        self.row_v.set_scroll_offset(self.state.y as u64);
        self.state.y = self.row_v.scroll_offset().min(u32::MAX as u64) as u32;
    }

    pub fn scroll_x_by(&mut self, delta: i32) {
        self.sync_virtualizers_from_state();
        self.state.scroll_x_by(delta);
        self.col_v.set_scroll_offset(self.state.x as u64);
        self.state.x = self.col_v.scroll_offset().min(u32::MAX as u64) as u32;
    }

    pub fn ensure_cursor_visible(&mut self) {
        self.sync_virtualizers_from_state();
        let Some(c) = self.cursor else {
            return;
        };
        self.row_v.scroll_to_index(c.row, Align::Auto);
        self.col_v.scroll_to_index(c.col, Align::Auto);
        self.state.y = self.row_v.scroll_offset().min(u32::MAX as u64) as u32;
        self.state.x = self.col_v.scroll_offset().min(u32::MAX as u64) as u32;
        self.state.clamp();
    }

    pub fn render<F>(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme, mut render_cell: F)
    where
        F: FnMut(Rect, DataGridCellContext, &mut Buffer, &Theme),
    {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let header_h = if self.options.show_header { 1u16 } else { 0u16 };
        let header_h = header_h.min(area.height);

        let (content_area, scrollbar_x) = if self.options.show_scrollbar_y && area.width >= 2 {
            (
                Rect::new(area.x, area.y, area.width - 1, area.height),
                Some(area.x + area.width - 1),
            )
        } else {
            (area, None)
        };

        let header_area = Rect::new(content_area.x, content_area.y, content_area.width, header_h);
        let body_area = Rect::new(
            content_area.x,
            content_area.y + header_h,
            content_area.width,
            content_area.height.saturating_sub(header_h),
        );

        let base_style = if self.options.style == Style::default() {
            theme.text_primary
        } else {
            self.options.style
        };
        let header_style = self.options.header_style.patch(theme.accent);
        let grid_line_style = if self.options.grid_line_style == Style::default() {
            theme.text_muted
        } else {
            self.options.grid_line_style
        };
        let cursor_style = self.options.cursor_style.patch(theme.accent);
        let selected_style = self.options.selected_style.patch(theme.accent);

        buf.set_style(content_area, base_style);
        buf.set_style(header_area, header_style);

        self.sync_virtualizers(body_area);
        self.collect_virtual_items();

        if header_area.height > 0 {
            self.render_header(
                header_area,
                buf,
                header_style,
                grid_line_style,
                &self.col_items,
            );
        }

        let mut render_ctx = RenderBodyContext {
            area: body_area,
            buf,
            row_items: &self.row_items,
            col_items: &self.col_items,
            theme,
        };
        self.render_body(
            &mut render_ctx,
            DataGridBodyStyles {
                base: base_style,
                cursor: cursor_style,
                selected: selected_style,
                grid_line: grid_line_style,
            },
            &mut render_cell,
        );

        if let Some(sb_x) = scrollbar_x {
            render::render_scrollbar(
                Rect::new(sb_x, body_area.y, 1, body_area.height),
                buf,
                &ViewportState {
                    x: 0,
                    y: self.state.y,
                    viewport_w: 1,
                    viewport_h: body_area.height,
                    content_w: 1,
                    content_h: self.state.content_h,
                },
                self.options.scrollbar_style,
            );
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> DataGridAction {
        if self.rows == 0 || self.columns.is_empty() {
            self.cursor = None;
            self.selection = Selection::None;
            self.selection_anchor = None;
            self.state.to_top();
            self.state.to_left();
            return DataGridAction::None;
        }

        self.sync_virtualizers_from_state();

        if key.modifiers.ctrl && !key.modifiers.alt {
            if matches!(key.code, KeyCode::Char('d')) {
                self.scroll_y_by(self.state.viewport_h.saturating_sub(1) as i32);
                self.cursor = Some(self.cursor_from_scroll());
                if self.options.selection_follows_cursor {
                    self.selection = self
                        .cursor
                        .map(Selection::Single)
                        .unwrap_or(Selection::None);
                }
                return DataGridAction::Redraw;
            }
            if matches!(key.code, KeyCode::Char('u')) {
                self.scroll_y_by(-(self.state.viewport_h.saturating_sub(1) as i32));
                self.cursor = Some(self.cursor_from_scroll());
                if self.options.selection_follows_cursor {
                    self.selection = self
                        .cursor
                        .map(Selection::Single)
                        .unwrap_or(Selection::None);
                }
                return DataGridAction::Redraw;
            }
        }

        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if self.move_cursor_by(1, 0, key.modifiers.shift) {
                    DataGridAction::Redraw
                } else {
                    DataGridAction::None
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.move_cursor_by(-1, 0, key.modifiers.shift) {
                    DataGridAction::Redraw
                } else {
                    DataGridAction::None
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if self.move_cursor_by(0, 1, key.modifiers.shift) {
                    DataGridAction::Redraw
                } else {
                    DataGridAction::None
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if self.move_cursor_by(0, -1, key.modifiers.shift) {
                    DataGridAction::Redraw
                } else {
                    DataGridAction::None
                }
            }
            KeyCode::PageDown => {
                self.scroll_y_by(self.state.viewport_h.saturating_sub(1) as i32);
                self.cursor = Some(self.cursor_from_scroll());
                if self.options.selection_follows_cursor {
                    self.selection = self
                        .cursor
                        .map(Selection::Single)
                        .unwrap_or(Selection::None);
                }
                DataGridAction::Redraw
            }
            KeyCode::PageUp => {
                self.scroll_y_by(-(self.state.viewport_h.saturating_sub(1) as i32));
                self.cursor = Some(self.cursor_from_scroll());
                if self.options.selection_follows_cursor {
                    self.selection = self
                        .cursor
                        .map(Selection::Single)
                        .unwrap_or(Selection::None);
                }
                DataGridAction::Redraw
            }
            KeyCode::Home => {
                self.set_cursor(Some(Cell { row: 0, col: 0 }));
                DataGridAction::Redraw
            }
            KeyCode::End => {
                self.set_cursor(Some(Cell {
                    row: self.rows.saturating_sub(1),
                    col: self.columns.len().saturating_sub(1),
                }));
                DataGridAction::Redraw
            }
            KeyCode::Char('g') => {
                let col = self.cursor.map(|c| c.col).unwrap_or(0);
                self.set_cursor(Some(Cell { row: 0, col }));
                DataGridAction::Redraw
            }
            KeyCode::Char('G') => {
                let col = self.cursor.map(|c| c.col).unwrap_or(0);
                self.set_cursor(Some(Cell {
                    row: self.rows.saturating_sub(1),
                    col,
                }));
                DataGridAction::Redraw
            }
            KeyCode::Char(' ') => {
                if let Some(c) = self.cursor {
                    self.selection = Selection::Single(c);
                    self.selection_anchor = Some(c);
                    DataGridAction::SelectionChanged
                } else {
                    DataGridAction::None
                }
            }
            KeyCode::Enter => self
                .cursor
                .map(DataGridAction::Activated)
                .unwrap_or(DataGridAction::None),
            _ => DataGridAction::None,
        }
    }

    fn move_cursor_by(&mut self, drow: i32, dcol: i32, shift: bool) -> bool {
        let cur = self.cursor.unwrap_or(Cell { row: 0, col: 0 });
        let next_row =
            (cur.row as i64 + drow as i64).clamp(0, self.rows.saturating_sub(1) as i64) as usize;
        let next_col = (cur.col as i64 + dcol as i64)
            .clamp(0, self.columns.len().saturating_sub(1) as i64) as usize;
        let next = Cell {
            row: next_row,
            col: next_col,
        };
        if Some(next) == self.cursor {
            return false;
        }
        self.cursor = Some(next);

        if self.options.multi_select && shift {
            let anchor = self.selection_anchor.unwrap_or(cur);
            self.selection_anchor = Some(anchor);
            self.selection = Selection::Rect {
                start: anchor,
                end: next,
            };
        } else if self.options.selection_follows_cursor {
            self.selection = Selection::Single(next);
            self.selection_anchor = Some(next);
        } else {
            self.selection_anchor = Some(next);
        }

        self.ensure_cursor_visible();
        true
    }

    fn cursor_from_scroll(&self) -> Cell {
        let row = self
            .row_v
            .index_at_offset(self.row_v.scroll_offset())
            .unwrap_or(0);
        let col = self
            .col_v
            .index_at_offset(self.col_v.scroll_offset())
            .unwrap_or(0);
        Cell { row, col }
    }

    fn sync_virtualizers(&mut self, body_area: Rect) {
        self.state.set_viewport(body_area.width, body_area.height);

        self.row_v.set_count(self.rows);
        self.row_v.set_viewport_size(body_area.height as u32);
        self.row_v.set_scroll_offset(self.state.y as u64);
        self.state.y = self.row_v.scroll_offset().min(u32::MAX as u64) as u32;

        self.col_v.set_count(self.columns.len());
        self.col_v.set_viewport_size(body_area.width as u32);
        self.col_v.set_scroll_offset(self.state.x as u64);
        self.state.x = self.col_v.scroll_offset().min(u32::MAX as u64) as u32;

        self.state
            .set_content(self.total_w_u32(), self.total_h_u32());
        self.state.clamp();

        self.row_v.set_overscan(self.options.overscan_rows);
        self.col_v.set_overscan(self.options.overscan_cols);
    }

    fn sync_virtualizers_from_state(&mut self) {
        self.row_v.set_count(self.rows);
        self.col_v.set_count(self.columns.len());

        self.row_v.set_viewport_size(self.state.viewport_h as u32);
        self.col_v.set_viewport_size(self.state.viewport_w as u32);

        self.row_v.set_scroll_offset(self.state.y as u64);
        self.col_v.set_scroll_offset(self.state.x as u64);

        self.state.y = self.row_v.scroll_offset().min(u32::MAX as u64) as u32;
        self.state.x = self.col_v.scroll_offset().min(u32::MAX as u64) as u32;

        self.state
            .set_content(self.total_w_u32(), self.total_h_u32());
        self.state.clamp();

        self.row_v.set_overscan(self.options.overscan_rows);
        self.col_v.set_overscan(self.options.overscan_cols);
    }

    fn total_h_u32(&self) -> u32 {
        self.row_v.total_size().min(u32::MAX as u64) as u32
    }

    fn total_w_u32(&self) -> u32 {
        self.col_v.total_size().min(u32::MAX as u64) as u32
    }

    fn rebuild_row_virtualizer(&mut self) {
        let row_height = self.options.row_height.max(1);
        let mut opts = VirtualizerOptions::new(self.rows, move |_| row_height);
        opts.overscan = self.options.overscan_rows;
        self.row_v = Virtualizer::new(opts);
        self.row_v.set_viewport_size(self.state.viewport_h as u32);
        self.row_v.set_scroll_offset(self.state.y as u64);
        self.state.y = self.row_v.scroll_offset().min(u32::MAX as u64) as u32;
    }

    fn rebuild_col_virtualizer(&mut self) {
        let widths: Vec<u32> = self.columns.iter().map(|c| c.width as u32).collect();
        let widths = std::sync::Arc::new(widths);
        let widths2 = widths.clone();
        let mut opts = VirtualizerOptions::new(self.columns.len(), move |i| {
            widths2.get(i).copied().unwrap_or(1).max(1)
        });
        opts.gap = self.options.col_gap;
        opts.overscan = self.options.overscan_cols;
        self.col_v = Virtualizer::new(opts);
        self.col_v.set_viewport_size(self.state.viewport_w as u32);
        self.col_v.set_scroll_offset(self.state.x as u64);
        self.state.x = self.col_v.scroll_offset().min(u32::MAX as u64) as u32;
    }

    fn collect_virtual_items(&mut self) {
        self.row_v.collect_virtual_items(&mut self.row_items);
        self.col_v.collect_virtual_items(&mut self.col_items);
    }

    fn render_header(
        &self,
        area: Rect,
        buf: &mut Buffer,
        style: Style,
        grid_line_style: Style,
        col_items: &[virtualizer::VirtualItem],
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        if self.columns.is_empty() {
            return;
        }
        buf.set_style(area, style);

        let scroll_x = self.col_v.scroll_offset();
        for col_item in col_items.iter().copied() {
            let col = &self.columns[col_item.index];
            let (rect, clip_left) = clipped_rect_x(area, scroll_x, col_item.start, col_item.size);
            if rect.width == 0 {
                continue;
            }
            render::render_str_clipped(
                rect.x, rect.y, clip_left, rect.width, buf, &col.title, style,
            );
            if self.options.col_gap > 0 {
                let mut ctx = ColSeparatorContext {
                    area,
                    scroll_x,
                    col_count: self.columns.len(),
                    buf,
                    style: grid_line_style,
                };
                maybe_draw_col_separator(&mut ctx, col_item.index, col_item.start, col_item.size);
            }
        }
    }

    fn render_body<F>(
        &self,
        render_ctx: &mut RenderBodyContext<'_>,
        styles: DataGridBodyStyles,
        render_cell: &mut F,
    ) where
        F: FnMut(Rect, DataGridCellContext, &mut Buffer, &Theme),
    {
        if render_ctx.area.width == 0 || render_ctx.area.height == 0 {
            return;
        }
        if self.rows == 0 || self.columns.is_empty() {
            return;
        }

        let scroll_x = self.col_v.scroll_offset();
        let scroll_y = self.row_v.scroll_offset();

        for row_item in render_ctx.row_items.iter().copied() {
            let (row_rect, clip_top) =
                clipped_rect_y(render_ctx.area, scroll_y, row_item.start, row_item.size);
            if row_rect.height == 0 {
                continue;
            }
            for col_item in render_ctx.col_items.iter().copied() {
                let (cell_rect, clip_left) =
                    clipped_rect_x(row_rect, scroll_x, col_item.start, col_item.size);
                if cell_rect.width == 0 || cell_rect.height == 0 {
                    continue;
                }

                let cell = Cell {
                    row: row_item.index,
                    col: col_item.index,
                };
                let is_cursor = self.cursor == Some(cell);
                let is_selected = self.selection.contains(cell);
                let style = if is_cursor {
                    styles.cursor
                } else if is_selected {
                    styles.selected
                } else {
                    styles.base
                };
                render_ctx.buf.set_style(cell_rect, style);

                let cell_ctx = DataGridCellContext {
                    cell,
                    col_width: self.columns[col_item.index].width,
                    row_start: row_item.start,
                    row_size: row_item.size,
                    col_start: col_item.start,
                    col_size: col_item.size,
                    clip_left,
                    clip_top,
                    is_cursor,
                    is_selected,
                };
                render_cell(cell_rect, cell_ctx, render_ctx.buf, render_ctx.theme);
                if self.options.col_gap > 0 {
                    let mut sep_ctx = ColSeparatorContext {
                        area: row_rect,
                        scroll_x,
                        col_count: self.columns.len(),
                        buf: render_ctx.buf,
                        style: styles.grid_line,
                    };
                    maybe_draw_col_separator(
                        &mut sep_ctx,
                        col_item.index,
                        col_item.start,
                        col_item.size,
                    );
                }
            }
        }
    }
}

fn clamp_cursor(cursor: Option<Cell>, rows: usize, cols: usize) -> Option<Cell> {
    if rows == 0 || cols == 0 {
        return None;
    }
    cursor.map(|c| Cell {
        row: c.row.min(rows - 1),
        col: c.col.min(cols - 1),
    })
}

fn clipped_rect_x(area: Rect, scroll_x: u64, start: u64, size: u32) -> (Rect, u32) {
    let rel = start as i64 - scroll_x as i64;
    let clip_left = (-rel).max(0) as u32;
    let x = rel.max(0) as u16;
    let max_w = area.width.saturating_sub(x);
    let visible_w = size.saturating_sub(clip_left).min(max_w as u32) as u16;
    (
        Rect::new(area.x + x, area.y, visible_w, area.height),
        clip_left,
    )
}

fn clipped_rect_y(area: Rect, scroll_y: u64, start: u64, size: u32) -> (Rect, u32) {
    let rel = start as i64 - scroll_y as i64;
    let clip_top = (-rel).max(0) as u32;
    let y = rel.max(0) as u16;
    let max_h = area.height.saturating_sub(y);
    let visible_h = size.saturating_sub(clip_top).min(max_h as u32) as u16;
    (
        Rect::new(area.x, area.y + y, area.width, visible_h),
        clip_top,
    )
}

fn maybe_draw_col_separator(
    ctx: &mut ColSeparatorContext<'_>,
    col_index: usize,
    col_start: u64,
    col_size: u32,
) {
    if col_index + 1 >= ctx.col_count {
        return;
    }
    let sep_x_rel = (col_start + col_size as u64) as i64 - ctx.scroll_x as i64;
    if sep_x_rel < 0 {
        return;
    }
    let sep_x = sep_x_rel as u16;
    if sep_x >= ctx.area.width {
        return;
    }
    for dy in 0..ctx.area.height {
        ctx.buf.set_span(
            ctx.area.x + sep_x,
            ctx.area.y + dy,
            &Span::styled("│", ctx.style),
            1,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::InputEvent;
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
    fn moves_cursor_and_scrolls_down() {
        let mut g = DataGridView::new();
        g.set_columns(vec![
            DataGridColumn::new("A", 5),
            DataGridColumn::new("B", 5),
            DataGridColumn::new("C", 5),
        ]);
        g.set_row_count(10_000);
        g.state.set_viewport(10, 5);
        g.set_cursor(Some(Cell { row: 0, col: 0 }));
        assert_eq!(g.state.y, 0);

        for _ in 0..10 {
            g.handle_event(InputEvent::Key(key(KeyCode::Down)));
        }
        assert_eq!(g.cursor(), Some(Cell { row: 10, col: 0 }));
        assert!(g.state.y > 0);
    }

    #[test]
    fn shift_selects_rect() {
        let mut g = DataGridView::with_options(DataGridViewOptions {
            multi_select: true,
            selection_follows_cursor: false,
            ..Default::default()
        });
        g.set_columns(vec![
            DataGridColumn::new("A", 5),
            DataGridColumn::new("B", 5),
            DataGridColumn::new("C", 5),
        ]);
        g.set_row_count(100);
        g.state.set_viewport(10, 5);
        g.set_cursor(Some(Cell { row: 2, col: 0 }));
        g.clear_selection();
        g.selection_anchor = Some(Cell { row: 2, col: 0 });

        g.handle_event(InputEvent::Key(key_shift(KeyCode::Down)));
        g.handle_event(InputEvent::Key(key_shift(KeyCode::Right)));
        assert!(matches!(g.selection(), Selection::Rect { .. }));
        assert!(g.selection().contains(Cell { row: 3, col: 1 }));
    }
}
