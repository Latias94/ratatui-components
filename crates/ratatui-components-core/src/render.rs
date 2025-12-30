use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Span;
use unicode_width::UnicodeWidthChar;

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

pub fn render_str_clipped(
    x: u16,
    y: u16,
    start_col: u32,
    max_cols: u16,
    buf: &mut Buffer,
    input: &str,
    style: Style,
) {
    if max_cols == 0 {
        return;
    }

    let start_col = start_col as usize;
    let max_cols = max_cols as usize;
    let mut col = 0usize;
    let mut out_cols = 0usize;
    let mut dx = 0u16;

    let mut tmp = [0u8; 4];

    for ch in input.chars() {
        if ch == '\t' {
            for _ in 0..4 {
                if col + 1 <= start_col {
                    col += 1;
                    continue;
                }
                if out_cols + 1 > max_cols {
                    return;
                }
                if let Some(cell) = buf.cell_mut((x + dx, y)) {
                    cell.set_style(style);
                    cell.set_symbol(" ");
                }
                dx += 1;
                out_cols += 1;
                col += 1;
            }
            continue;
        }

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

        let s = ch.encode_utf8(&mut tmp);
        if let Some(cell) = buf.cell_mut((x + dx, y)) {
            cell.set_style(style);
            cell.set_symbol(s);
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
    let mut tmp = [0u8; 4];

    for span in spans {
        let style = if span.style == Style::default() {
            fallback_style
        } else {
            span.style
        };
        for ch in span.content.chars() {
            if ch == '\t' {
                for _ in 0..4 {
                    if col + 1 <= start_col {
                        col += 1;
                        continue;
                    }
                    if out_cols + 1 > max_cols {
                        return;
                    }
                    if let Some(cell) = buf.cell_mut((x + dx, y)) {
                        cell.set_style(style);
                        cell.set_symbol(" ");
                    }
                    dx += 1;
                    out_cols += 1;
                    col += 1;
                }
                continue;
            }

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

            let s = ch.encode_utf8(&mut tmp);
            if let Some(cell) = buf.cell_mut((x + dx, y)) {
                cell.set_style(style);
                cell.set_symbol(s);
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
        out_cols += w;
    }

    out
}

pub fn byte_range_for_cols(input: &str, start_col: u32, end_col: u32) -> Option<(usize, usize)> {
    let start_col = start_col as usize;
    let end_col = end_col as usize;

    if start_col >= end_col {
        return None;
    }

    let mut col = 0usize;
    let mut start_b: Option<usize> = None;
    let mut end_b: Option<usize> = None;

    for (b, ch) in input.char_indices() {
        if ch == '\t' {
            for _ in 0..4 {
                if col == start_col && start_b.is_none() {
                    start_b = Some(b);
                }
                if col == end_col && end_b.is_none() {
                    end_b = Some(b);
                }
                col += 1;
            }
            if start_b.is_some() && end_b.is_some() {
                break;
            }
            continue;
        }

        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w == 0 {
            continue;
        }

        if col == start_col && start_b.is_none() {
            start_b = Some(b);
        }
        if col == end_col && end_b.is_none() {
            end_b = Some(b);
        }

        if col < start_col && col + w > start_col {
            start_b = Some(b + ch.len_utf8());
        }
        if col < end_col && col + w > end_col {
            end_b = Some(b + ch.len_utf8());
        }

        col += w;
        if start_b.is_some() && end_b.is_some() {
            break;
        }
        if col > end_col {
            break;
        }
    }

    let start_b = start_b.unwrap_or(input.len());
    let end_b = end_b.unwrap_or(input.len());
    if start_b >= end_b {
        None
    } else {
        Some((start_b, end_b))
    }
}

pub fn byte_range_for_cols_in_spans(
    spans: &[Span<'static>],
    start_col: u32,
    end_col: u32,
) -> Option<(usize, usize)> {
    let start_col = start_col as usize;
    let end_col = end_col as usize;
    if start_col >= end_col {
        return None;
    }

    let mut col = 0usize;
    let mut start_b: Option<usize> = None;
    let mut end_b: Option<usize> = None;
    let mut global_b = 0usize;

    for span in spans {
        let s = span.content.as_ref();
        for (local_b, ch) in s.char_indices() {
            let abs_b = global_b + local_b;

            if start_b.is_none() && col == start_col {
                start_b = Some(abs_b);
            }
            if end_b.is_none() && col == end_col {
                end_b = Some(abs_b);
            }

            let w = UnicodeWidthChar::width(ch).unwrap_or(0);
            if w == 0 {
                continue;
            }

            if col < start_col && col + w > start_col {
                start_b = Some(abs_b + ch.len_utf8());
            }
            if col < end_col && col + w > end_col {
                end_b = Some(abs_b + ch.len_utf8());
            }

            col += w;

            if start_b.is_some() && end_b.is_some() {
                break;
            }
            if col > end_col {
                break;
            }
        }
        global_b += s.len();
        if start_b.is_some() && end_b.is_some() {
            break;
        }
    }

    let total = global_b;
    let start_b = start_b.unwrap_or(total);
    let end_b = end_b.unwrap_or(total);
    if start_b >= end_b {
        None
    } else {
        Some((start_b, end_b))
    }
}

pub fn slice_spans_by_bytes(spans: &[Span<'static>], start_b: usize, end_b: usize) -> String {
    if start_b >= end_b {
        return String::new();
    }

    let mut out = String::new();
    let mut global = 0usize;

    for span in spans {
        let s = span.content.as_ref();
        let span_start = global;
        let span_end = global + s.len();

        if end_b <= span_start {
            break;
        }
        if start_b >= span_end {
            global = span_end;
            continue;
        }

        let lo = start_b.saturating_sub(span_start).min(s.len());
        let hi = end_b.saturating_sub(span_start).min(s.len());
        if lo < hi {
            out.push_str(&s[lo..hi]);
        }

        global = span_end;
    }

    out
}

pub fn apply_modifier_to_byte_ranges(
    spans: Vec<Span<'static>>,
    ranges: &[(usize, usize)],
    modifier: Modifier,
) -> Vec<Span<'static>> {
    if ranges.is_empty() {
        return spans;
    }

    let mut out: Vec<Span<'static>> = Vec::new();
    let mut range_idx = 0usize;
    let mut global = 0usize;

    for span in spans {
        let s = span.content.as_ref();
        let span_len = s.len();
        if span_len == 0 {
            out.push(span);
            continue;
        }

        let mut local = 0usize;
        while range_idx < ranges.len() && ranges[range_idx].1 <= global {
            range_idx += 1;
        }

        while range_idx < ranges.len() {
            let (rs, re) = ranges[range_idx];
            if rs >= global + span_len {
                break;
            }

            let start_in = rs.saturating_sub(global).min(span_len);
            let end_in = re.saturating_sub(global).min(span_len);

            if start_in > local {
                out.push(Span::styled(s[local..start_in].to_string(), span.style));
            }
            if end_in > start_in {
                out.push(Span::styled(
                    s[start_in..end_in].to_string(),
                    span.style.add_modifier(modifier),
                ));
            }

            local = end_in;
            if re <= global + span_len {
                range_idx += 1;
            } else {
                break;
            }
        }

        if local < span_len {
            out.push(Span::styled(s[local..].to_string(), span.style));
        }

        global += span_len;
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
    fn render_spans_clipped_expands_tabs() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, 1));
        let spans = vec![Span::raw("\t1")];
        render_spans_clipped(0, 0, 0, 6, &mut buf, &spans, Style::default());
        let s: String = (0..6)
            .map(|x| buf.cell((x, 0)).unwrap().symbol().to_string())
            .collect();
        assert!(s.starts_with("    1"));
    }

    #[test]
    fn byte_range_for_cols_skips_partial_wide_chars() {
        let s = "你好";
        assert_eq!(
            byte_range_for_cols(s, 0, 2).map(|(a, b)| &s[a..b]),
            Some("你")
        );
        assert_eq!(
            byte_range_for_cols(s, 2, 4).map(|(a, b)| &s[a..b]),
            Some("好")
        );
        assert_eq!(
            byte_range_for_cols(s, 1, 3).map(|(a, b)| &s[a..b]),
            Some("好")
        );
    }

    #[test]
    fn byte_range_for_cols_in_spans_works_across_boundaries() {
        let spans = vec![Span::raw("ab"), Span::raw("cd")];
        let (a, b) = byte_range_for_cols_in_spans(&spans, 1, 3).unwrap();
        assert_eq!(slice_spans_by_bytes(&spans, a, b), "bc");
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
