use crate::input::KeyCode;
use crate::input::KeyEvent;
use crate::keymap;
use crate::viewport::ViewportState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollAction {
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Top,
    Bottom,
}

#[derive(Clone, Debug)]
pub struct ScrollBindings {
    pub line_step: i32,
    pub horiz_step: i32,
    pub up: Vec<KeyEvent>,
    pub down: Vec<KeyEvent>,
    pub left: Vec<KeyEvent>,
    pub right: Vec<KeyEvent>,
    pub page_up: Vec<KeyEvent>,
    pub page_down: Vec<KeyEvent>,
    pub top: Vec<KeyEvent>,
    pub bottom: Vec<KeyEvent>,
}

impl Default for ScrollBindings {
    fn default() -> Self {
        Self {
            line_step: 1,
            horiz_step: 4,
            up: vec![KeyEvent::new(KeyCode::Up), keymap::key_char('k')],
            down: vec![KeyEvent::new(KeyCode::Down), keymap::key_char('j')],
            left: vec![KeyEvent::new(KeyCode::Left), keymap::key_char('h')],
            right: vec![KeyEvent::new(KeyCode::Right), keymap::key_char('l')],
            page_up: vec![KeyEvent::new(KeyCode::PageUp), keymap::key_ctrl('u')],
            page_down: vec![KeyEvent::new(KeyCode::PageDown), keymap::key_ctrl('d')],
            top: vec![KeyEvent::new(KeyCode::Home), keymap::key_char('g')],
            bottom: vec![KeyEvent::new(KeyCode::End), keymap::key_char('G')],
        }
    }
}

impl ScrollBindings {
    pub fn action_for(&self, key: &KeyEvent) -> Option<ScrollAction> {
        if self.up.iter().any(|p| keymap::key_event_matches(p, key)) {
            return Some(ScrollAction::Up);
        }
        if self.down.iter().any(|p| keymap::key_event_matches(p, key)) {
            return Some(ScrollAction::Down);
        }
        if self.left.iter().any(|p| keymap::key_event_matches(p, key)) {
            return Some(ScrollAction::Left);
        }
        if self.right.iter().any(|p| keymap::key_event_matches(p, key)) {
            return Some(ScrollAction::Right);
        }
        if self
            .page_up
            .iter()
            .any(|p| keymap::key_event_matches(p, key))
        {
            return Some(ScrollAction::PageUp);
        }
        if self
            .page_down
            .iter()
            .any(|p| keymap::key_event_matches(p, key))
        {
            return Some(ScrollAction::PageDown);
        }
        if self.top.iter().any(|p| keymap::key_event_matches(p, key)) {
            return Some(ScrollAction::Top);
        }
        if self
            .bottom
            .iter()
            .any(|p| keymap::key_event_matches(p, key))
        {
            return Some(ScrollAction::Bottom);
        }
        None
    }

    pub fn apply(&self, state: &mut ViewportState, action: ScrollAction) {
        match action {
            ScrollAction::Up => state.scroll_y_by(-self.line_step),
            ScrollAction::Down => state.scroll_y_by(self.line_step),
            ScrollAction::Left => state.scroll_x_by(-self.horiz_step),
            ScrollAction::Right => state.scroll_x_by(self.horiz_step),
            ScrollAction::PageUp => state.page_up(),
            ScrollAction::PageDown => state.page_down(),
            ScrollAction::Top => state.to_top(),
            ScrollAction::Bottom => state.to_bottom(),
        }
    }
}
