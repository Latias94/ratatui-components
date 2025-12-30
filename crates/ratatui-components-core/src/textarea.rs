use crate::input::InputEvent;
use crate::input::KeyCode;
use crate::input::KeyEvent;
use crate::render;
use crate::viewport::ViewportState;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnterBehavior {
    Newline,
    Submit,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SubmitRule {
    Never,
    #[default]
    EnterSubmitsShiftNewline,
    ShiftEnterSubmitsEnterNewline,
}

#[derive(Clone, Debug)]
pub struct TextAreaOptions {
    pub show_scrollbar: bool,
    pub style: Style,
    pub submit_rule: SubmitRule,
}

impl Default for TextAreaOptions {
    fn default() -> Self {
        Self {
            show_scrollbar: true,
            style: Style::default(),
            submit_rule: SubmitRule::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    pub row: usize,
    pub col: usize, // char index within line
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextAreaAction {
    None,
    Changed,
    Submitted(String),
}

#[derive(Clone, Debug)]
pub struct TextArea {
    lines: Vec<String>,
    cursor: Cursor,
    preferred_x: Option<usize>, // display columns
    pub state: ViewportState,
    options: TextAreaOptions,
}

impl Default for TextArea {
    fn default() -> Self {
        Self::new()
    }
}

impl TextArea {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor: Cursor::default(),
            preferred_x: None,
            state: ViewportState::default(),
            options: TextAreaOptions::default(),
        }
    }

    pub fn with_options(options: TextAreaOptions) -> Self {
        Self {
            options,
            ..Self::new()
        }
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = normalize_newlines(&text.into());
        self.lines = split_lines_keep_trailing(&text);
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor = Cursor::default();
        self.preferred_x = None;
        self.recompute_content_size();
        self.state.clamp();
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn set_viewport(&mut self, area: Rect) {
        let content_area = if self.options.show_scrollbar && area.width >= 2 {
            Rect::new(area.x, area.y, area.width - 1, area.height)
        } else {
            area
        };
        self.state
            .set_viewport(content_area.width, content_area.height);
        self.recompute_content_size();
        self.ensure_cursor_visible();
    }

    pub fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        if area.width == 0 || area.height == 0 {
            return None;
        }
        let content_area = if self.options.show_scrollbar && area.width >= 2 {
            Rect::new(area.x, area.y, area.width - 1, area.height)
        } else {
            area
        };
        let (cx, cy) = self.cursor_screen_pos();
        let x = cx.saturating_sub(self.state.x);
        let y = cy.saturating_sub(self.state.y);
        if x >= content_area.width as u32 || y >= content_area.height as u32 {
            return None;
        }
        Some((
            content_area.x + x.min(u16::MAX as u32) as u16,
            content_area.y + y.min(u16::MAX as u32) as u16,
        ))
    }

    pub fn input(&mut self, event: InputEvent) -> TextAreaAction {
        match event {
            InputEvent::Paste(s) => {
                self.insert_str(&s);
                self.preferred_x = None;
                self.recompute_content_size();
                self.ensure_cursor_visible();
                TextAreaAction::Changed
            }
            InputEvent::Key(key) => self.handle_key(key),
            InputEvent::Mouse(_) => TextAreaAction::None,
        }
    }

    pub fn render_ref(&mut self, area: Rect, buf: &mut Buffer) {
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
                self.options.style,
            );
            if let Some(line) = self.lines.get(idx) {
                render::render_str_clipped(
                    content_area.x,
                    y,
                    self.state.x,
                    content_area.width,
                    buf,
                    line,
                    self.options.style,
                );
            }
        }

        if let Some(sb_x) = scrollbar_x {
            render::render_scrollbar(
                Rect::new(sb_x, area.y, 1, area.height),
                buf,
                &self.state,
                self.options.style,
            );
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> TextAreaAction {
        match key.code {
            KeyCode::Char(c) => {
                if key.modifiers.ctrl || key.modifiers.alt {
                    return TextAreaAction::None;
                }
                self.insert_char(c);
                self.preferred_x = None;
                self.recompute_content_size();
                self.ensure_cursor_visible();
                TextAreaAction::Changed
            }
            KeyCode::Enter => match self.enter_behavior(key) {
                EnterBehavior::Newline => {
                    self.insert_newline();
                    self.preferred_x = None;
                    self.recompute_content_size();
                    self.ensure_cursor_visible();
                    TextAreaAction::Changed
                }
                EnterBehavior::Submit => {
                    let submitted = self.text();
                    self.set_text("");
                    TextAreaAction::Submitted(submitted)
                }
            },
            KeyCode::Backspace => {
                if self.backspace() {
                    self.preferred_x = None;
                    self.recompute_content_size();
                    self.ensure_cursor_visible();
                    TextAreaAction::Changed
                } else {
                    TextAreaAction::None
                }
            }
            KeyCode::Delete => {
                if self.delete() {
                    self.preferred_x = None;
                    self.recompute_content_size();
                    self.ensure_cursor_visible();
                    TextAreaAction::Changed
                } else {
                    TextAreaAction::None
                }
            }
            KeyCode::Left => {
                self.move_left();
                self.ensure_cursor_visible();
                TextAreaAction::None
            }
            KeyCode::Right => {
                self.move_right();
                self.ensure_cursor_visible();
                TextAreaAction::None
            }
            KeyCode::Up => {
                self.move_up();
                self.ensure_cursor_visible();
                TextAreaAction::None
            }
            KeyCode::Down => {
                self.move_down();
                self.ensure_cursor_visible();
                TextAreaAction::None
            }
            KeyCode::Home => {
                self.cursor.col = 0;
                self.preferred_x = Some(0);
                self.ensure_cursor_visible();
                TextAreaAction::None
            }
            KeyCode::End => {
                self.cursor.col = self.current_line_char_len();
                self.preferred_x = Some(self.cursor_display_x());
                self.ensure_cursor_visible();
                TextAreaAction::None
            }
            KeyCode::PageDown => {
                self.state.page_down();
                TextAreaAction::None
            }
            KeyCode::PageUp => {
                self.state.page_up();
                TextAreaAction::None
            }
            KeyCode::Tab | KeyCode::Esc => TextAreaAction::None,
        }
    }

    fn enter_behavior(&self, key: KeyEvent) -> EnterBehavior {
        match self.options.submit_rule {
            SubmitRule::Never => EnterBehavior::Newline,
            SubmitRule::EnterSubmitsShiftNewline => {
                if key.modifiers.shift {
                    EnterBehavior::Newline
                } else {
                    EnterBehavior::Submit
                }
            }
            SubmitRule::ShiftEnterSubmitsEnterNewline => {
                if key.modifiers.shift {
                    EnterBehavior::Submit
                } else {
                    EnterBehavior::Newline
                }
            }
        }
    }

    fn recompute_content_size(&mut self) {
        let content_h = self.lines.len() as u32;
        let content_w = self
            .lines
            .iter()
            .map(|l| UnicodeWidthStr::width(l.as_str()) as u32)
            .max()
            .unwrap_or(0);
        self.state.set_content(content_w, content_h);
    }

    fn ensure_cursor_visible(&mut self) {
        let (cx, cy) = self.cursor_screen_pos();
        if cy < self.state.y {
            self.state.y = cy;
        } else if cy >= self.state.y.saturating_add(self.state.viewport_h as u32) {
            self.state.y = cy.saturating_sub(self.state.viewport_h.saturating_sub(1) as u32);
        }

        if cx < self.state.x {
            self.state.x = cx;
        } else if cx >= self.state.x.saturating_add(self.state.viewport_w as u32) {
            self.state.x = cx.saturating_sub(self.state.viewport_w.saturating_sub(1) as u32);
        }

        self.state.clamp();
    }

    fn cursor_screen_pos(&self) -> (u32, u32) {
        let y = self.cursor.row.min(self.lines.len().saturating_sub(1)) as u32;
        let x = self.cursor_display_x() as u32;
        (x, y)
    }

    fn cursor_display_x(&self) -> usize {
        let line = self.current_line();
        let mut cols = 0usize;
        for (i, ch) in line.chars().enumerate() {
            if i >= self.cursor.col {
                break;
            }
            cols += UnicodeWidthChar::width(ch).unwrap_or(0);
        }
        cols
    }

    fn current_line(&self) -> &str {
        self.lines
            .get(self.cursor.row)
            .map(String::as_str)
            .unwrap_or("")
    }

    fn current_line_char_len(&self) -> usize {
        self.current_line().chars().count()
    }

    fn move_left(&mut self) {
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
        } else if self.cursor.row > 0 {
            self.cursor.row -= 1;
            self.cursor.col = self.current_line_char_len();
        }
        self.preferred_x = Some(self.cursor_display_x());
    }

    fn move_right(&mut self) {
        let len = self.current_line_char_len();
        if self.cursor.col < len {
            self.cursor.col += 1;
        } else if self.cursor.row + 1 < self.lines.len() {
            self.cursor.row += 1;
            self.cursor.col = 0;
        }
        self.preferred_x = Some(self.cursor_display_x());
    }

    fn move_up(&mut self) {
        if self.cursor.row == 0 {
            return;
        }
        let target_x = self.preferred_x.unwrap_or_else(|| self.cursor_display_x());
        self.cursor.row -= 1;
        self.cursor.col = col_from_display_x(self.current_line(), target_x);
        self.preferred_x = Some(target_x);
    }

    fn move_down(&mut self) {
        if self.cursor.row + 1 >= self.lines.len() {
            return;
        }
        let target_x = self.preferred_x.unwrap_or_else(|| self.cursor_display_x());
        self.cursor.row += 1;
        self.cursor.col = col_from_display_x(self.current_line(), target_x);
        self.preferred_x = Some(target_x);
    }

    fn insert_char(&mut self, ch: char) {
        let row = self.cursor.row.min(self.lines.len() - 1);
        let line = &mut self.lines[row];
        let byte_idx = byte_index_from_char_index(line, self.cursor.col);
        line.insert(byte_idx, ch);
        self.cursor.row = row;
        self.cursor.col += 1;
    }

    fn insert_newline(&mut self) {
        let row = self.cursor.row.min(self.lines.len() - 1);
        let line = &mut self.lines[row];
        let byte_idx = byte_index_from_char_index(line, self.cursor.col);
        let tail = line[byte_idx..].to_string();
        line.truncate(byte_idx);
        self.lines.insert(row + 1, tail);
        self.cursor.row = row + 1;
        self.cursor.col = 0;
    }

    fn insert_str(&mut self, s: &str) {
        let s = normalize_newlines(s);
        let parts: Vec<&str> = s.split('\n').collect();
        if parts.is_empty() {
            return;
        }
        if parts.len() == 1 {
            for ch in parts[0].chars() {
                self.insert_char(ch);
            }
            return;
        }

        let row = self.cursor.row.min(self.lines.len() - 1);
        let byte_idx = byte_index_from_char_index(&self.lines[row], self.cursor.col);
        let tail = self.lines[row][byte_idx..].to_string();
        self.lines[row].truncate(byte_idx);
        self.lines[row].push_str(parts[0]);

        let mut insert_at = row + 1;
        for mid in &parts[1..parts.len() - 1] {
            self.lines.insert(insert_at, (*mid).to_string());
            insert_at += 1;
        }

        let last = parts.last().unwrap_or(&"");
        self.lines.insert(insert_at, format!("{last}{tail}"));

        self.cursor.row = insert_at;
        self.cursor.col = last.chars().count();
    }

    fn backspace(&mut self) -> bool {
        if self.is_empty() {
            return false;
        }
        if self.cursor.col > 0 {
            let row = self.cursor.row.min(self.lines.len() - 1);
            let line = &mut self.lines[row];
            let start = byte_index_from_char_index(line, self.cursor.col - 1);
            let end = byte_index_from_char_index(line, self.cursor.col);
            line.replace_range(start..end, "");
            self.cursor.row = row;
            self.cursor.col -= 1;
            return true;
        }
        if self.cursor.row > 0 {
            let row = self.cursor.row;
            let cur = self.lines.remove(row);
            self.cursor.row -= 1;
            let prev = &mut self.lines[self.cursor.row];
            let prev_len = prev.chars().count();
            prev.push_str(&cur);
            self.cursor.col = prev_len;
            if self.lines.is_empty() {
                self.lines.push(String::new());
                self.cursor = Cursor::default();
            }
            return true;
        }
        false
    }

    fn delete(&mut self) -> bool {
        if self.is_empty() {
            return false;
        }
        let row = self.cursor.row.min(self.lines.len() - 1);
        let line_len = self.lines[row].chars().count();
        if self.cursor.col < line_len {
            let line = &mut self.lines[row];
            let start = byte_index_from_char_index(line, self.cursor.col);
            let end = byte_index_from_char_index(line, self.cursor.col + 1);
            line.replace_range(start..end, "");
            return true;
        }
        if row + 1 < self.lines.len() {
            let next = self.lines.remove(row + 1);
            self.lines[row].push_str(&next);
            return true;
        }
        false
    }
}

fn col_from_display_x(line: &str, target_x: usize) -> usize {
    let mut cols = 0usize;
    let mut col = 0usize;
    for ch in line.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if cols + w > target_x {
            break;
        }
        cols += w;
        col += 1;
    }
    col
}

fn byte_index_from_char_index(s: &str, char_idx: usize) -> usize {
    if char_idx == 0 {
        return 0;
    }
    match s.char_indices().nth(char_idx) {
        Some((i, _)) => i,
        None => s.len(),
    }
}

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

fn split_lines_keep_trailing(s: &str) -> Vec<String> {
    if s.is_empty() {
        return vec![String::new()];
    }
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for ch in s.chars() {
        if ch == '\n' {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(ch);
        }
    }
    out.push(cur);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::InputEvent;
    use crate::input::KeyCode;
    use crate::input::KeyEvent;
    use crate::input::KeyModifiers;

    #[test]
    fn inserts_and_moves_cursor() {
        let mut ta = TextArea::new();
        assert_eq!(
            ta.input(InputEvent::Key(KeyEvent::new(KeyCode::Char('a')))),
            TextAreaAction::Changed
        );
        assert_eq!(ta.text(), "a");
        assert_eq!(ta.cursor(), Cursor { row: 0, col: 1 });
        ta.input(InputEvent::Key(KeyEvent::new(KeyCode::Left)));
        assert_eq!(ta.cursor(), Cursor { row: 0, col: 0 });
        ta.input(InputEvent::Key(KeyEvent::new(KeyCode::Char('b'))));
        assert_eq!(ta.text(), "ba");
    }

    #[test]
    fn enter_submits_by_default() {
        let mut ta = TextArea::new();
        ta.input(InputEvent::Key(KeyEvent::new(KeyCode::Char('x'))));
        let act = ta.input(InputEvent::Key(KeyEvent::new(KeyCode::Enter)));
        assert_eq!(act, TextAreaAction::Submitted("x".to_string()));
        assert_eq!(ta.text(), "");
    }

    #[test]
    fn shift_enter_inserts_newline_by_default() {
        let mut ta = TextArea::new();
        ta.input(InputEvent::Key(KeyEvent::new(KeyCode::Char('x'))));
        let key = KeyEvent::new(KeyCode::Enter).with_modifiers(KeyModifiers {
            shift: true,
            ctrl: false,
            alt: false,
        });
        let act = ta.input(InputEvent::Key(key));
        assert_eq!(act, TextAreaAction::Changed);
        assert_eq!(ta.text(), "x\n");
    }

    #[test]
    fn backspace_joins_lines() {
        let mut ta = TextArea::new();
        ta.set_text("a\nb");
        ta.cursor = Cursor { row: 1, col: 0 };
        assert_eq!(
            ta.input(InputEvent::Key(KeyEvent::new(KeyCode::Backspace))),
            TextAreaAction::Changed
        );
        assert_eq!(ta.text(), "ab");
        assert_eq!(ta.cursor(), Cursor { row: 0, col: 1 });
    }

    #[test]
    fn paste_multiline_inserts() {
        let mut ta = TextArea::new();
        ta.input(InputEvent::Paste("a\nb\nc".to_string()));
        assert_eq!(ta.text(), "a\nb\nc");
        assert_eq!(ta.cursor.row, 2);
    }
}
