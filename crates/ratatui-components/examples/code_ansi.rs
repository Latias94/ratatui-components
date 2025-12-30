use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEventKind;
use crossterm::event::MouseEventKind;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::style::Stylize;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui_components::ansi::AnsiTextView;
use ratatui_components::code_view::CodeView;
use ratatui_components::help::HelpBar;
use ratatui_components::help::HelpBarOptions;
use ratatui_components::input::InputEvent;
use ratatui_components::input::MouseButton;
use ratatui_components::input::MouseEvent;
use ratatui_components::keymap;
use ratatui_components::keymap::Binding;
use ratatui_components::syntax::syntect::SyntectHighlighter;
use ratatui_components::theme::Theme;
use std::io;
use std::sync::Arc;
use std::time::Duration;

const SAMPLE_CODE: &str = r#"fn main() {
    let answer = 42;
    println!("answer = {}", answer);
}
"#;

const SAMPLE_ANSI: &str = "\u{1b}[1mBold\u{1b}[0m normal  \u{1b}[31mred\u{1b}[0m\n\
tab:\tcol2\n\
long line: 0123456789012345678901234567890123456789\n\
";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
    Code,
    Ansi,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Quit,
    FocusNext,
    ScrollDown,
    ScrollUp,
    ScrollLeft,
    ScrollRight,
    Top,
    Bottom,
    PageDown,
    PageUp,
}

fn main() -> io::Result<()> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let theme = Theme::default();
    let highlighter = Arc::new(SyntectHighlighter::new());

    let mut code = CodeView::new();
    code.set_language(Some("rs"));
    code.set_code(SAMPLE_CODE);
    code.set_highlighter(Some(highlighter));

    let mut ansi = AnsiTextView::new();
    ansi.set_ansi(SAMPLE_ANSI);

    let mut focus = Focus::Code;
    let keymap = build_keymap();
    let help = build_help_bar(&keymap);

    let res = run(
        &mut terminal,
        &theme,
        &help,
        &keymap,
        &mut code,
        &mut ansi,
        &mut focus,
    );

    disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    res
}

#[derive(Clone, Copy, Debug, Default)]
struct LayoutState {
    code: ratatui::layout::Rect,
    ansi: ratatui::layout::Rect,
}

fn run<B: ratatui::backend::Backend<Error = io::Error>>(
    terminal: &mut Terminal<B>,
    theme: &Theme,
    help: &HelpBar,
    keymap: &[BindingAction],
    code: &mut CodeView,
    ansi: &mut AnsiTextView,
    focus: &mut Focus,
) -> io::Result<()> {
    let mut layout = LayoutState::default();
    loop {
        terminal.draw(|f| ui(f, theme, help, code, ansi, *focus, &mut layout))?;

        if !crossterm::event::poll(Duration::from_millis(50))? {
            continue;
        }

        match crossterm::event::read()? {
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                if let Some(action) = map_action(keymap, &key)
                    && handle_action(action, focus, code, ansi)
                {
                    return Ok(());
                }
            }
            Event::Mouse(m) => {
                let Some(ev) = to_mouse_event(m) else {
                    continue;
                };
                if point_in(layout.code, ev.x, ev.y) {
                    let _ = code.handle_event_action_in_area(layout.code, InputEvent::Mouse(ev));
                } else if point_in(layout.ansi, ev.x, ev.y) {
                    let _ = ansi.handle_event_action_in_area(layout.ansi, InputEvent::Mouse(ev));
                }
            }
            _ => {}
        }
    }
}

fn ui(
    f: &mut ratatui::Frame<'_>,
    theme: &Theme,
    help: &HelpBar,
    code: &mut CodeView,
    ansi: &mut AnsiTextView,
    focus: Focus,
    layout: &mut LayoutState,
) {
    let area = f.area();
    let [top, status] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .areas(area);

    let [left, right] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .areas(top);

    let left_title = match focus {
        Focus::Code => "CodeView (focused)".cyan().bold(),
        Focus::Ansi => "CodeView".cyan(),
    };
    let right_title = match focus {
        Focus::Ansi => "AnsiTextView (focused)".cyan().bold(),
        Focus::Code => "AnsiTextView".cyan(),
    };

    let left_block = Block::default().title(left_title).borders(Borders::ALL);
    let right_block = Block::default().title(right_title).borders(Borders::ALL);

    let left_inner = left_block.inner(left);
    let right_inner = right_block.inner(right);

    layout.code = left_inner;
    layout.ansi = right_inner;

    f.render_widget(left_block, left);
    f.render_widget(right_block, right);

    let buf = f.buffer_mut();
    code.render_ref(left_inner, buf, theme);
    ansi.render_ref(right_inner, buf, theme);

    help.render_ref(status, buf);
}

struct BindingAction {
    binding: Binding,
    action: Action,
}

fn build_keymap() -> Vec<BindingAction> {
    use Action::*;
    vec![
        BindingAction {
            binding: Binding::new("q", "quit", vec![keymap::key_char('q')]),
            action: Quit,
        },
        BindingAction {
            binding: Binding::new("Tab", "focus", vec![ratatui_key(KeyCode::Tab)]),
            action: FocusNext,
        },
        BindingAction {
            binding: Binding::new(
                "j/↓",
                "down",
                vec![keymap::key_char('j'), ratatui_key(KeyCode::Down)],
            ),
            action: ScrollDown,
        },
        BindingAction {
            binding: Binding::new(
                "k/↑",
                "up",
                vec![keymap::key_char('k'), ratatui_key(KeyCode::Up)],
            ),
            action: ScrollUp,
        },
        BindingAction {
            binding: Binding::new(
                "h/←",
                "left",
                vec![keymap::key_char('h'), ratatui_key(KeyCode::Left)],
            ),
            action: ScrollLeft,
        },
        BindingAction {
            binding: Binding::new(
                "l/→",
                "right",
                vec![keymap::key_char('l'), ratatui_key(KeyCode::Right)],
            ),
            action: ScrollRight,
        },
        BindingAction {
            binding: Binding::new("g", "top", vec![keymap::key_char('g')]),
            action: Top,
        },
        BindingAction {
            binding: Binding::new("G", "bottom", vec![keymap::key_char('G')]),
            action: Bottom,
        },
        BindingAction {
            binding: Binding::new("PgDn", "page down", vec![ratatui_key(KeyCode::PageDown)]),
            action: PageDown,
        },
        BindingAction {
            binding: Binding::new("PgUp", "page up", vec![ratatui_key(KeyCode::PageUp)]),
            action: PageUp,
        },
    ]
}

fn build_help_bar(keymap: &[BindingAction]) -> HelpBar {
    HelpBar::with_options(
        keymap.iter().map(|b| b.binding.clone()).collect(),
        HelpBarOptions {
            key_style: theme_key_style(),
            ..Default::default()
        },
    )
}

fn theme_key_style() -> ratatui::style::Style {
    ratatui::style::Style::default().bold()
}

fn map_action(keymap: &[BindingAction], key: &crossterm::event::KeyEvent) -> Option<Action> {
    let ev = to_key_event(key)?;
    keymap
        .iter()
        .find(|b| b.binding.matches(&ev))
        .map(|b| b.action)
}

fn handle_action(
    action: Action,
    focus: &mut Focus,
    code: &mut CodeView,
    ansi: &mut AnsiTextView,
) -> bool {
    match action {
        Action::Quit => true,
        Action::FocusNext => {
            *focus = match focus {
                Focus::Code => Focus::Ansi,
                Focus::Ansi => Focus::Code,
            };
            false
        }
        Action::ScrollDown => {
            match focus {
                Focus::Code => code.scroll_y_by(1),
                Focus::Ansi => ansi.scroll_y_by(1),
            }
            false
        }
        Action::ScrollUp => {
            match focus {
                Focus::Code => code.scroll_y_by(-1),
                Focus::Ansi => ansi.scroll_y_by(-1),
            }
            false
        }
        Action::ScrollLeft => {
            match focus {
                Focus::Code => code.scroll_x_by(-4),
                Focus::Ansi => ansi.scroll_x_by(-4),
            }
            false
        }
        Action::ScrollRight => {
            match focus {
                Focus::Code => code.scroll_x_by(4),
                Focus::Ansi => ansi.scroll_x_by(4),
            }
            false
        }
        Action::Top => {
            match focus {
                Focus::Code => code.state.to_top(),
                Focus::Ansi => ansi.state.to_top(),
            }
            false
        }
        Action::Bottom => {
            match focus {
                Focus::Code => code.state.to_bottom(),
                Focus::Ansi => ansi.state.to_bottom(),
            }
            false
        }
        Action::PageDown => {
            match focus {
                Focus::Code => code.state.page_down(),
                Focus::Ansi => ansi.state.page_down(),
            }
            false
        }
        Action::PageUp => {
            match focus {
                Focus::Code => code.state.page_up(),
                Focus::Ansi => ansi.state.page_up(),
            }
            false
        }
    }
}

fn to_key_event(key: &crossterm::event::KeyEvent) -> Option<ratatui_components::input::KeyEvent> {
    use ratatui_components::input::KeyCode as K;
    use ratatui_components::input::KeyEvent;
    use ratatui_components::input::KeyModifiers;

    let code = match key.code {
        KeyCode::Char(c) => K::Char(c),
        KeyCode::Enter => K::Enter,
        KeyCode::Backspace => K::Backspace,
        KeyCode::Delete => K::Delete,
        KeyCode::Tab => K::Tab,
        KeyCode::Esc => K::Esc,
        KeyCode::Left => K::Left,
        KeyCode::Right => K::Right,
        KeyCode::Up => K::Up,
        KeyCode::Down => K::Down,
        KeyCode::Home => K::Home,
        KeyCode::End => K::End,
        KeyCode::PageUp => K::PageUp,
        KeyCode::PageDown => K::PageDown,
        _ => return None,
    };

    Some(
        KeyEvent::new(code).with_modifiers(KeyModifiers {
            shift: key
                .modifiers
                .contains(crossterm::event::KeyModifiers::SHIFT),
            ctrl: key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL),
            alt: key.modifiers.contains(crossterm::event::KeyModifiers::ALT),
        }),
    )
}

fn ratatui_key(code: KeyCode) -> ratatui_components::input::KeyEvent {
    use ratatui_components::input::KeyCode as K;
    use ratatui_components::input::KeyEvent;
    match code {
        KeyCode::Tab => KeyEvent::new(K::Tab),
        KeyCode::Up => KeyEvent::new(K::Up),
        KeyCode::Down => KeyEvent::new(K::Down),
        KeyCode::Left => KeyEvent::new(K::Left),
        KeyCode::Right => KeyEvent::new(K::Right),
        KeyCode::PageUp => KeyEvent::new(K::PageUp),
        KeyCode::PageDown => KeyEvent::new(K::PageDown),
        _ => KeyEvent::new(K::Esc),
    }
}

fn point_in(r: ratatui::layout::Rect, x: u16, y: u16) -> bool {
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}

fn to_mouse_event(m: crossterm::event::MouseEvent) -> Option<MouseEvent> {
    let kind = match m.kind {
        MouseEventKind::Down(b) => {
            ratatui_components::input::MouseEventKind::Down(to_mouse_button(b)?)
        }
        MouseEventKind::Drag(b) => {
            ratatui_components::input::MouseEventKind::Drag(to_mouse_button(b)?)
        }
        MouseEventKind::Up(b) => ratatui_components::input::MouseEventKind::Up(to_mouse_button(b)?),
        MouseEventKind::ScrollUp => ratatui_components::input::MouseEventKind::ScrollUp,
        MouseEventKind::ScrollDown => ratatui_components::input::MouseEventKind::ScrollDown,
        _ => return None,
    };

    Some(MouseEvent {
        x: m.column,
        y: m.row,
        kind,
        modifiers: ratatui_components::input::KeyModifiers {
            shift: m.modifiers.contains(crossterm::event::KeyModifiers::SHIFT),
            ctrl: m
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL),
            alt: m.modifiers.contains(crossterm::event::KeyModifiers::ALT),
        },
    })
}

fn to_mouse_button(b: crossterm::event::MouseButton) -> Option<MouseButton> {
    match b {
        crossterm::event::MouseButton::Left => Some(MouseButton::Left),
        crossterm::event::MouseButton::Right => Some(MouseButton::Right),
        crossterm::event::MouseButton::Middle => Some(MouseButton::Middle),
    }
}
