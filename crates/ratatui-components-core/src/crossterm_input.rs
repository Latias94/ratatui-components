use crate::input::InputEvent;
use crate::input::KeyCode;
use crate::input::KeyEvent;
use crate::input::KeyModifiers;
use crate::input::MouseButton;
use crate::input::MouseEvent;
use crate::input::MouseEventKind;

pub fn input_event_from_crossterm(ev: crossterm::event::Event) -> Option<InputEvent> {
    match ev {
        crossterm::event::Event::Key(key) => {
            if key.kind != crossterm::event::KeyEventKind::Press {
                return None;
            }
            Some(InputEvent::Key(key_event_from_crossterm(key)?))
        }
        crossterm::event::Event::Paste(s) => Some(InputEvent::Paste(s)),
        crossterm::event::Event::Mouse(m) => {
            Some(InputEvent::Mouse(mouse_event_from_crossterm(m)?))
        }
        _ => None,
    }
}

pub fn key_event_from_crossterm(key: crossterm::event::KeyEvent) -> Option<KeyEvent> {
    let code = match key.code {
        crossterm::event::KeyCode::Char(c) => KeyCode::Char(c),
        crossterm::event::KeyCode::Enter => KeyCode::Enter,
        crossterm::event::KeyCode::Backspace => KeyCode::Backspace,
        crossterm::event::KeyCode::Delete => KeyCode::Delete,
        crossterm::event::KeyCode::Tab => KeyCode::Tab,
        crossterm::event::KeyCode::Esc => KeyCode::Esc,
        crossterm::event::KeyCode::Left => KeyCode::Left,
        crossterm::event::KeyCode::Right => KeyCode::Right,
        crossterm::event::KeyCode::Up => KeyCode::Up,
        crossterm::event::KeyCode::Down => KeyCode::Down,
        crossterm::event::KeyCode::Home => KeyCode::Home,
        crossterm::event::KeyCode::End => KeyCode::End,
        crossterm::event::KeyCode::PageUp => KeyCode::PageUp,
        crossterm::event::KeyCode::PageDown => KeyCode::PageDown,
        _ => return None,
    };

    Some(KeyEvent {
        code,
        modifiers: modifiers_from_crossterm(key.modifiers),
    })
}

pub fn mouse_event_from_crossterm(m: crossterm::event::MouseEvent) -> Option<MouseEvent> {
    let kind = match m.kind {
        crossterm::event::MouseEventKind::Down(b) => {
            MouseEventKind::Down(mouse_button_from_crossterm(b)?)
        }
        crossterm::event::MouseEventKind::Drag(b) => {
            MouseEventKind::Drag(mouse_button_from_crossterm(b)?)
        }
        crossterm::event::MouseEventKind::Up(b) => {
            MouseEventKind::Up(mouse_button_from_crossterm(b)?)
        }
        crossterm::event::MouseEventKind::ScrollUp => MouseEventKind::ScrollUp,
        crossterm::event::MouseEventKind::ScrollDown => MouseEventKind::ScrollDown,
        _ => return None,
    };

    Some(MouseEvent {
        x: m.column,
        y: m.row,
        kind,
        modifiers: modifiers_from_crossterm(m.modifiers),
    })
}

fn modifiers_from_crossterm(m: crossterm::event::KeyModifiers) -> KeyModifiers {
    KeyModifiers {
        shift: m.contains(crossterm::event::KeyModifiers::SHIFT),
        ctrl: m.contains(crossterm::event::KeyModifiers::CONTROL),
        alt: m.contains(crossterm::event::KeyModifiers::ALT),
    }
}

fn mouse_button_from_crossterm(b: crossterm::event::MouseButton) -> Option<MouseButton> {
    match b {
        crossterm::event::MouseButton::Left => Some(MouseButton::Left),
        crossterm::event::MouseButton::Right => Some(MouseButton::Right),
        crossterm::event::MouseButton::Middle => Some(MouseButton::Middle),
    }
}
