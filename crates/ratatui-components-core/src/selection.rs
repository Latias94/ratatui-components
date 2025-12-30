use crate::input::KeyCode;
use crate::input::KeyEvent;
use crate::keymap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionAction {
    None,
    Redraw,
    CopyRequested(String),
}

#[derive(Clone, Debug)]
pub struct SelectionBindings {
    pub copy: Vec<KeyEvent>,
    pub clear: Vec<KeyEvent>,
}

impl Default for SelectionBindings {
    fn default() -> Self {
        Self {
            copy: vec![keymap::key_char('y')],
            clear: vec![KeyEvent::new(KeyCode::Esc)],
        }
    }
}

impl SelectionBindings {
    pub fn is_copy(&self, key: &KeyEvent) -> bool {
        self.copy.iter().any(|p| keymap::key_event_matches(p, key))
    }

    pub fn is_clear(&self, key: &KeyEvent) -> bool {
        self.clear.iter().any(|p| keymap::key_event_matches(p, key))
    }
}
