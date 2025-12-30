use ansi_to_tui::IntoText;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::Line;
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
use crate::theme::Theme;
use crate::viewport::ViewportState;

fn expand_tabs(s: &str) -> std::borrow::Cow<'_, str> {
    if s.contains('\t') {
        std::borrow::Cow::Owned(s.replace('\t', "    "))
    } else {
        std::borrow::Cow::Borrowed(s)
    }
}

/// Converts an ANSI-colored string into a [`Text`].
///
/// Tabs are expanded as 4 spaces before parsing to keep hit-testing and selection consistent.
pub fn ansi_text(input: &str) -> Text<'static> {
    let input = expand_tabs(input);
    input
        .as_ref()
        .into_text()
        .unwrap_or_else(|_| Text::from(input.to_string()))
}

/// Options for [`AnsiTextView`].
#[derive(Clone, Debug)]
pub struct AnsiTextViewOptions {
    pub show_scrollbar: bool,
    pub scroll: ScrollBindings,
    pub enable_selection: bool,
    pub selection: SelectionBindings,
}

impl Default for AnsiTextViewOptions {
    fn default() -> Self {
        Self {
            show_scrollbar: true,
            scroll: ScrollBindings::default(),
            enable_selection: true,
            selection: SelectionBindings::default(),
        }
    }
}

/// A scrollable ANSI text viewer with optional mouse selection + copy-on-request.
///
/// Selection is performed in terminal cell units (line, column). When the copy binding is pressed,
/// the view returns a [`SelectionAction::CopyRequested`] containing the extracted plain text.
///
/// This widget intentionally does not interact with the system clipboard.
#[derive(Clone, Default)]
pub struct AnsiTextView {
    lines: Vec<Line<'static>>,
    max_content_width: u16,
    pub state: ViewportState,
    options: AnsiTextViewOptions,
    selection_anchor: Option<(usize, u32)>,
    selection: Option<((usize, u32), (usize, u32))>,
}

impl AnsiTextView {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_options(options: AnsiTextViewOptions) -> Self {
        Self {
            options,
            ..Self::default()
        }
    }

    /// Sets the ANSI input and updates internal content metrics.
    pub fn set_ansi(&mut self, input: &str) {
        let text = ansi_text(input);
        self.lines = text.lines;
        self.max_content_width = self.lines.iter().map(line_width).max().unwrap_or(0);
        self.state
            .set_content(self.max_content_width as u32, self.lines.len() as u32);
    }

    /// Handles an event and returns `true` if a redraw is needed.
    pub fn handle_event(&mut self, event: InputEvent) -> bool {
        !matches!(self.handle_event_action(event), SelectionAction::None)
    }

    /// Handles an event and returns a [`SelectionAction`] (redraw / copy-on-request).
    pub fn handle_event_action(&mut self, event: InputEvent) -> SelectionAction {
        match event {
            InputEvent::Paste(_) => SelectionAction::None,
            InputEvent::Mouse(m) => match m.kind {
                MouseEventKind::ScrollUp => {
                    self.state.scroll_y_by(-3);
                    SelectionAction::Redraw
                }
                MouseEventKind::ScrollDown => {
                    self.state.scroll_y_by(3);
                    SelectionAction::Redraw
                }
                _ => SelectionAction::None,
            },
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

    /// Like [`Self::handle_event`], but first updates viewport state for `area`.
    pub fn handle_event_in_area(&mut self, area: Rect, event: InputEvent) -> bool {
        !matches!(
            self.handle_event_action_in_area(area, event),
            SelectionAction::None
        )
    }

    /// Like [`Self::handle_event_action`], but first updates viewport state for `area`.
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

    /// Handles mouse events for scrolling and drag-selection.
    pub fn handle_mouse_event(&mut self, area: Rect, event: MouseEvent) -> bool {
        if area.width == 0 || area.height == 0 {
            return false;
        }

        match event.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                return !matches!(
                    self.handle_event_action(InputEvent::Mouse(event)),
                    SelectionAction::None
                );
            }
            _ => {}
        }

        if !self.options.enable_selection {
            return false;
        }
        if self.lines.is_empty() {
            return false;
        }

        let content_area = if self.options.show_scrollbar && area.width >= 2 {
            Rect::new(area.x, area.y, area.width - 1, area.height)
        } else {
            area
        };

        let content_start_x = content_area.x;
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

        let rel_x = (x - content_area.x) as u32;
        let rel_y = (y - content_area.y) as u32;
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
            let line = self.lines.get(line_idx)?;
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

            if let Some((bs, be)) = render::byte_range_for_cols_in_spans(&line.spans, from, to) {
                out.push_str(&render::slice_spans_by_bytes(&line.spans, bs, be));
            }
        }
        Some(out)
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
        self.state
            .set_viewport(content_area.width, content_area.height);
    }

    pub fn render_ref(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        self.set_viewport(area);

        let (content_area, scrollbar_x) = if self.options.show_scrollbar && area.width >= 2 {
            (
                Rect::new(area.x, area.y, area.width - 1, area.height),
                Some(area.x + area.width - 1),
            )
        } else {
            (area, None)
        };

        for row in 0..content_area.height {
            let y = content_area.y + row;
            let idx = (self.state.y as usize).saturating_add(row as usize);
            buf.set_style(
                Rect::new(content_area.x, y, content_area.width, 1),
                theme.text_primary,
            );
            if let Some(line) = self.lines.get(idx) {
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
                            render::byte_range_for_cols_in_spans(&line.spans, from, to)
                        {
                            let spans = render::apply_modifier_to_byte_ranges(
                                line.spans.clone(),
                                &[(bs, be)],
                                Modifier::REVERSED,
                            );
                            render::render_spans_clipped(
                                content_area.x,
                                y,
                                self.state.x,
                                content_area.width,
                                buf,
                                &spans,
                                theme.text_primary,
                            );
                            continue;
                        }
                    }
                }

                render::render_spans_clipped(
                    content_area.x,
                    y,
                    self.state.x,
                    content_area.width,
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
                &self.state,
                theme.text_muted,
            );
        }
    }
}

fn line_width(line: &Line<'static>) -> u16 {
    let w: u32 = line
        .spans
        .iter()
        .map(|s| UnicodeWidthStr::width(s.content.as_ref()) as u32)
        .sum();
    w.min(u16::MAX as u32) as u16
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
    use ratatui::buffer::Buffer;

    #[test]
    fn ansi_text_view_renders_without_panic() {
        let mut v = AnsiTextView::new();
        v.set_ansi("\u{1b}[31mred\u{1b}[0m\ttext\nsecond line\n");
        let theme = Theme::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 2));
        v.render_ref(Rect::new(0, 0, 10, 2), &mut buf, &theme);
    }
}
