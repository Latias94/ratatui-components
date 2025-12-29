use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, Default)]
pub struct ViewportState {
    pub x: u16,
    pub y: u16,
    pub viewport_w: u16,
    pub viewport_h: u16,
    pub content_w: u16,
    pub content_h: u16,
}

impl ViewportState {
    pub fn set_viewport(&mut self, w: u16, h: u16) {
        self.viewport_w = w;
        self.viewport_h = h;
        self.clamp();
    }

    pub fn set_content(&mut self, w: u16, h: u16) {
        self.content_w = w;
        self.content_h = h;
        self.clamp();
    }

    pub fn clamp(&mut self) {
        let max_y = self.content_h.saturating_sub(self.viewport_h);
        let max_x = self.content_w.saturating_sub(self.viewport_w);
        self.y = self.y.min(max_y);
        self.x = self.x.min(max_x);
    }

    pub fn scroll_y_by(&mut self, delta: i16) {
        let next = self.y as i32 + delta as i32;
        self.y = next.clamp(0, self.max_y() as i32) as u16;
    }

    pub fn scroll_x_by(&mut self, delta: i16) {
        let next = self.x as i32 + delta as i32;
        self.x = next.clamp(0, self.max_x() as i32) as u16;
    }

    pub fn page_down(&mut self) {
        self.scroll_y_by(self.viewport_h.saturating_sub(1) as i16);
    }

    pub fn page_up(&mut self) {
        self.scroll_y_by(-(self.viewport_h.saturating_sub(1) as i16));
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
        if self.content_h == 0 || self.viewport_h == 0 || self.content_h <= self.viewport_h {
            return None;
        }
        let visible_bottom = self.y.saturating_add(self.viewport_h) as f32;
        let pct = (visible_bottom / self.content_h as f32 * 100.0).round();
        Some(pct.clamp(0.0, 100.0) as u8)
    }

    fn max_y(&self) -> u16 {
        self.content_h.saturating_sub(self.viewport_h)
    }

    fn max_x(&self) -> u16 {
        self.content_w.saturating_sub(self.viewport_w)
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
        let idx = state.y as usize + row as usize;
        buf.set_style(Rect::new(text_area.x, y, text_area.width, 1), options.style);
        if let Some(line) = lines.get(idx) {
            let visible = slice_by_cols(line, state.x, text_area.width);
            buf.set_stringn(
                text_area.x,
                y,
                visible,
                text_area.width as usize,
                options.style,
            );
        }
    }

    if let Some(sb_x) = scrollbar_x {
        render_scrollbar(
            Rect::new(sb_x, area.y, 1, area.height),
            buf,
            state,
            options.scrollbar_style,
        );
    }
}

fn render_scrollbar(area: Rect, buf: &mut Buffer, state: &ViewportState, style: Style) {
    buf.set_style(area, style);
    if area.height == 0 {
        return;
    }
    if state.content_h <= state.viewport_h || state.content_h == 0 {
        for dy in 0..area.height {
            buf.set_stringn(area.x, area.y + dy, " ", 1, style);
        }
        return;
    }

    let track_h = area.height as f32;
    let thumb_h = ((state.viewport_h as f32 / state.content_h as f32) * track_h)
        .round()
        .clamp(1.0, track_h) as u16;

    let max_y = state.content_h.saturating_sub(state.viewport_h).max(1) as f32;
    let thumb_top = ((state.y as f32 / max_y) * (track_h - thumb_h as f32))
        .round()
        .clamp(0.0, (track_h - thumb_h as f32).max(0.0)) as u16;

    for dy in 0..area.height {
        let ch = if dy >= thumb_top && dy < thumb_top + thumb_h {
            "█"
        } else {
            " "
        };
        buf.set_stringn(area.x, area.y + dy, ch, 1, style);
    }
}

fn slice_by_cols(input: &str, start_col: u16, max_cols: u16) -> String {
    if max_cols == 0 {
        return String::new();
    }

    let start_col = start_col as usize;
    let max_cols = max_cols as usize;
    let input = if input.contains('\t') {
        std::borrow::Cow::Owned(input.replace('\t', "    "))
    } else {
        std::borrow::Cow::Borrowed(input)
    };
    let mut col = 0usize;
    let mut out_cols = 0usize;
    let mut out = String::new();

    for ch in input.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w == 0 {
            continue;
        }
        if col + w <= start_col {
            col += w;
            continue;
        }
        if col < start_col && col + w > start_col {
            col += w;
            continue;
        }
        if out_cols + w > max_cols {
            break;
        }
        out.push(ch);
        col += w;
        out_cols = UnicodeWidthStr::width(out.as_str());
    }

    out
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
        assert_eq!(slice_by_cols("\t1", 0, 4), "    ");
        assert_eq!(slice_by_cols("abcdef", 0, 3), "abc");
        assert_eq!(slice_by_cols("abcdef", 2, 3), "cde");
    }

    #[test]
    fn slice_by_cols_skips_partial_wide_char_overlap() {
        assert_eq!(slice_by_cols("你好", 0, 2), "你");
        assert_eq!(slice_by_cols("你好", 2, 2), "好");
        assert_eq!(slice_by_cols("你好", 1, 2), "好");
    }
}
