use crate::render;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

#[derive(Clone, Copy, Debug, Default)]
pub struct ViewportState {
    pub x: u32,
    pub y: u32,
    pub viewport_w: u16,
    pub viewport_h: u16,
    pub content_w: u32,
    pub content_h: u32,
}

impl ViewportState {
    pub fn set_viewport(&mut self, w: u16, h: u16) {
        self.viewport_w = w;
        self.viewport_h = h;
        self.clamp();
    }

    pub fn set_content(&mut self, w: u32, h: u32) {
        self.content_w = w;
        self.content_h = h;
        self.clamp();
    }

    pub fn clamp(&mut self) {
        let max_y = self.max_y();
        let max_x = self.max_x();
        self.y = self.y.min(max_y);
        self.x = self.x.min(max_x);
    }

    pub fn scroll_y_by(&mut self, delta: i32) {
        let next = self.y as i64 + delta as i64;
        self.y = next.clamp(0, self.max_y() as i64) as u32;
    }

    pub fn scroll_x_by(&mut self, delta: i32) {
        let next = self.x as i64 + delta as i64;
        self.x = next.clamp(0, self.max_x() as i64) as u32;
    }

    pub fn page_down(&mut self) {
        self.scroll_y_by(self.viewport_h.saturating_sub(1) as i32);
    }

    pub fn page_up(&mut self) {
        self.scroll_y_by(-(self.viewport_h.saturating_sub(1) as i32));
    }

    pub fn to_top(&mut self) {
        self.y = 0;
    }

    pub fn to_bottom(&mut self) {
        self.y = self.max_y();
    }

    pub fn to_left(&mut self) {
        self.x = 0;
    }

    pub fn to_right(&mut self) {
        self.x = self.max_x();
    }

    pub fn percent_y(&self) -> Option<u8> {
        if self.content_h == 0 || self.viewport_h == 0 || self.content_h <= self.viewport_h as u32 {
            return None;
        }
        let visible_bottom = self.y.saturating_add(self.viewport_h as u32) as f64;
        let pct = (visible_bottom / self.content_h as f64 * 100.0).round();
        Some(pct.clamp(0.0, 100.0) as u8)
    }

    fn max_y(&self) -> u32 {
        self.content_h.saturating_sub(self.viewport_h as u32)
    }

    fn max_x(&self) -> u32 {
        self.content_w.saturating_sub(self.viewport_w as u32)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ViewportOptions {
    pub style: Style,
    pub show_scrollbar: bool,
    pub scrollbar_style: Style,
}

pub fn render_lines(area: Rect, buf: &mut Buffer, lines: &[String], state: &ViewportState) {
    render_lines_with_options(area, buf, lines, state, &ViewportOptions::default())
}

pub fn render_lines_with_options(
    area: Rect,
    buf: &mut Buffer,
    lines: &[String],
    state: &ViewportState,
    options: &ViewportOptions,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let (text_area, scrollbar_x) = if options.show_scrollbar && area.width >= 2 {
        (
            Rect::new(area.x, area.y, area.width - 1, area.height),
            Some(area.x + area.width - 1),
        )
    } else {
        (area, None)
    };

    for row in 0..text_area.height {
        let y = row + text_area.y;
        let idx = (state.y as usize).saturating_add(row as usize);
        buf.set_style(Rect::new(text_area.x, y, text_area.width, 1), options.style);
        if let Some(line) = lines.get(idx) {
            render::render_str_clipped(
                text_area.x,
                y,
                state.x,
                text_area.width,
                buf,
                line,
                options.style,
            );
        }
    }

    if let Some(sb_x) = scrollbar_x {
        render::render_scrollbar(
            Rect::new(sb_x, area.y, 1, area.height),
            buf,
            state,
            options.scrollbar_style,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_clamps_both_axes() {
        let mut s = ViewportState::default();
        s.set_viewport(10, 5);
        s.set_content(12, 6);
        s.x = 99;
        s.y = 99;
        s.clamp();
        assert_eq!(s.x, 2);
        assert_eq!(s.y, 1);
    }

    #[test]
    fn slice_by_cols_handles_tabs_and_limits_width() {
        assert_eq!(render::slice_by_cols("\t1", 0, 4), "    ");
        assert_eq!(render::slice_by_cols("abcdef", 0, 3), "abc");
        assert_eq!(render::slice_by_cols("abcdef", 2, 3), "cde");
    }

    #[test]
    fn slice_by_cols_skips_partial_wide_char_overlap() {
        assert_eq!(render::slice_by_cols("你好", 0, 2), "你");
        assert_eq!(render::slice_by_cols("你好", 2, 2), "好");
        assert_eq!(render::slice_by_cols("你好", 1, 2), "好");
    }
}
