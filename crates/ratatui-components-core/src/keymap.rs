use crate::input::KeyCode;
use crate::input::KeyEvent;
use crate::input::KeyModifiers;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub keys: Vec<KeyEvent>,
    pub help_key: String,
    pub help_desc: String,
}

impl Binding {
    pub fn new(
        help_key: impl Into<String>,
        help_desc: impl Into<String>,
        keys: Vec<KeyEvent>,
    ) -> Self {
        Self {
            keys,
            help_key: help_key.into(),
            help_desc: help_desc.into(),
        }
    }

    pub fn matches(&self, event: &KeyEvent) -> bool {
        self.keys.iter().any(|k| key_event_matches(k, event))
    }
}

pub fn key_event_matches(pattern: &KeyEvent, event: &KeyEvent) -> bool {
    pattern.code == event.code && modifiers_match(pattern.modifiers, event.modifiers)
}

fn modifiers_match(pattern: KeyModifiers, event: KeyModifiers) -> bool {
    pattern.shift == event.shift && pattern.ctrl == event.ctrl && pattern.alt == event.alt
}

pub fn key_char(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c))
}

pub fn key_ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c)).with_modifiers(KeyModifiers {
        shift: false,
        ctrl: true,
        alt: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_matches_exact_modifiers() {
        let b = Binding::new("q", "quit", vec![key_char('q')]);
        assert!(b.matches(&key_char('q')));
        assert!(!b.matches(&key_ctrl('q')));
    }
}
