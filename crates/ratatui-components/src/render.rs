use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

use crate::viewport::ViewportState;

pub fn render_scrollbar(area: Rect, buf: &mut Buffer, state: &ViewportState, style: Style) {
    buf.set_style(area, style);
    if area.height == 0 {
        return;
    }
    if state.content_h <= state.viewport_h as u32 || state.content_h == 0 {
        for dy in 0..area.height {
            buf.set_stringn(area.x, area.y + dy, " ", 1, style);
        }
        return;
    }

    let track_h = area.height as f64;
    let thumb_h = ((state.viewport_h as f64 / state.content_h as f64) * track_h)
        .round()
        .clamp(1.0, track_h) as u16;

    let max_y = state
        .content_h
        .saturating_sub(state.viewport_h as u32)
        .max(1) as f64;
    let thumb_top = ((state.y as f64 / max_y) * (track_h - thumb_h as f64))
        .round()
        .clamp(0.0, (track_h - thumb_h as f64).max(0.0)) as u16;

    for dy in 0..area.height {
        let ch = if dy >= thumb_top && dy < thumb_top + thumb_h {
            "█"
        } else {
            " "
        };
        buf.set_stringn(area.x, area.y + dy, ch, 1, style);
    }
}

pub fn render_spans_clipped(
    x: u16,
    y: u16,
    start_col: u32,
    max_cols: u16,
    buf: &mut Buffer,
    spans: &[Span<'static>],
    fallback_style: Style,
) {
    if max_cols == 0 {
        return;
    }

    let start_col = start_col as usize;
    let max_cols = max_cols as usize;
    let mut col = 0usize;
    let mut out_cols = 0usize;
    let mut dx = 0u16;

    for span in spans {
        let style = if span.style == Style::default() {
            fallback_style
        } else {
            span.style
        };
        for ch in span.content.chars() {
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
                return;
            }

            if let Some(cell) = buf.cell_mut((x + dx, y)) {
                cell.set_style(style);
                cell.set_symbol(&ch.to_string());
            }
            dx += 1;
            out_cols += 1;
            col += w;

            if w == 2 {
                if out_cols >= max_cols {
                    return;
                }
                if let Some(cell) = buf.cell_mut((x + dx, y)) {
                    cell.set_style(style);
                    cell.set_symbol("");
                }
                dx += 1;
                out_cols += 1;
            }
        }
    }
}

pub fn slice_by_cols(input: &str, start_col: u32, max_cols: u16) -> String {
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
    use ratatui::buffer::Buffer;

    #[test]
    fn slice_by_cols_handles_tabs_and_width() {
        assert_eq!(slice_by_cols("\t1", 0, 4), "    ");
        assert_eq!(slice_by_cols("abcdef", 0, 3), "abc");
        assert_eq!(slice_by_cols("abcdef", 2, 3), "cde");
    }

    #[test]
    fn render_scrollbar_does_not_panic() {
        let mut state = ViewportState::default();
        state.set_viewport(10, 5);
        state.set_content(10, 50);
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 5));
        render_scrollbar(Rect::new(0, 0, 1, 5), &mut buf, &state, Style::default());
    }
}
