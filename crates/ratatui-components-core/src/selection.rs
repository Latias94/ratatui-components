use crate::input::KeyCode;
use crate::input::KeyEvent;
use crate::keymap;

/// Actions produced by selection-capable widgets.
///
/// This crate intentionally does not integrate with any system clipboard. Instead, widgets emit a
/// `CopyRequested(String)` action and let the app decide what to do (copy to clipboard, write to a
/// file, send over IPC, etc).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionAction {
    None,
    Redraw,
    CopyRequested(String),
}

/// Key bindings for selection interactions.
///
/// The defaults are intentionally Vim-like:
/// - `y` requests copying the current selection
/// - `Esc` clears the selection
///
/// Widgets typically check these bindings in their `handle_event_action*` method and return a
/// [`SelectionAction::CopyRequested`] so callers can plug in a clipboard implementation.
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
    /// Returns `true` if `key` matches any configured copy binding.
    pub fn is_copy(&self, key: &KeyEvent) -> bool {
        self.copy.iter().any(|p| keymap::key_event_matches(p, key))
    }

    /// Returns `true` if `key` matches any configured clear-selection binding.
    pub fn is_clear(&self, key: &KeyEvent) -> bool {
        self.clear.iter().any(|p| keymap::key_event_matches(p, key))
    }
}
