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
use ratatui_components::diff::DiffView;
use ratatui_components::input::InputEvent;
use ratatui_components::input::KeyCode as K;
use ratatui_components::input::KeyEvent as KE;
use ratatui_components::input::KeyModifiers as KM;
use ratatui_components::input::MouseButton;
use ratatui_components::input::MouseEvent;
use ratatui_components::selection::SelectionAction;
use ratatui_components::syntax::syntect::SyntectHighlighter;
use ratatui_components::textarea::TextArea;
use ratatui_components::textarea::TextAreaAction;
use ratatui_components::theme::Theme;
use ratatui_components::transcript::view::Role;
use ratatui_components::transcript::view::TranscriptView;
use std::io;
use std::time::Duration;

const SAMPLE_DIFF: &str = r#"
diff --git a/main.rs b/main.rs
index 0000000..1111111 100644
--- a/main.rs
+++ b/main.rs
@@ -1,3 +1,8 @@
 fn main() {
-    println!("Hello, world!");
+    println!("Hello, ratatui-components!");
+    println!("DiffView supports scrolling.");
 }
+fn helper() {
+    // a very long line that should require horizontal scrolling in narrow terminals:
+    println!("012345678901234567890123456789012345678901234567890123456789");
+}
"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
    Transcript,
    Diff,
    Input,
}

fn main() -> io::Result<()> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let theme = Theme::default();
    let highlighter = std::sync::Arc::new(SyntectHighlighter::new());

    let mut transcript = TranscriptView::new();
    transcript.set_highlighter(Some(highlighter.clone()));
    transcript.push_markdown(
        Role::System,
        "# MVP demo\n\nLeft: transcript (Markdown + ANSI)\nRight: diff preview\nBottom: composer\n\nPress `Tab` to switch focus, `q` to quit.",
    );
    transcript.push_ansi(
        Role::Tool,
        "\u{1b}[32mtool\u{1b}[0m: running...\n\u{1b}[31merror\u{1b}[0m: something happened\n",
    );
    transcript.push_markdown(
        Role::Assistant,
        "This is **Markdown**. Code blocks are supported:\n\n```rs\nfn main() {\n    println!(\"hi\");\n}\n```",
    );

    let mut diff = DiffView::new();
    diff.set_diff(SAMPLE_DIFF.trim());
    diff.set_highlighter(Some(highlighter));
    diff.set_language_override(Some("rs"));

    let mut input = TextArea::new();
    let mut focus = Focus::Input;
    let mut copied: Option<String> = None;

    let res = run(
        &mut terminal,
        &theme,
        &mut transcript,
        &mut diff,
        &mut input,
        &mut focus,
        &mut copied,
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

#[allow(clippy::too_many_arguments)]
fn run<B: ratatui::backend::Backend<Error = io::Error>>(
    terminal: &mut Terminal<B>,
    theme: &Theme,
    transcript: &mut TranscriptView,
    diff: &mut DiffView,
    input: &mut TextArea,
    focus: &mut Focus,
    copied: &mut Option<String>,
) -> io::Result<()> {
    let mut layout = LayoutState::default();
    loop {
        terminal.draw(|f| {
            let cursor = ui(
                f,
                theme,
                transcript,
                diff,
                input,
                *focus,
                copied,
                &mut layout,
            );
            if let Some((x, y)) = cursor {
                f.set_cursor_position((x, y));
            }
        })?;

        if !crossterm::event::poll(Duration::from_millis(50))? {
            continue;
        }

        match crossterm::event::read()? {
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                if key.modifiers.is_empty() && matches!(key.code, KeyCode::Char('q')) {
                    return Ok(());
                }

                if matches!(key.code, KeyCode::Tab) {
                    *focus = match focus {
                        Focus::Transcript => Focus::Diff,
                        Focus::Diff => Focus::Input,
                        Focus::Input => Focus::Transcript,
                    };
                    continue;
                }

                match *focus {
                    Focus::Input => {
                        if let Some(ev) = to_input_event(key) {
                            match input.input(ev) {
                                TextAreaAction::Submitted(text) => {
                                    if !text.trim().is_empty() {
                                        transcript.push_markdown(Role::User, &text);
                                        transcript.push_markdown(
                                            Role::Assistant,
                                            "_tip: use `Tab` to focus the transcript/diff panes and scroll._",
                                        );
                                    }
                                }
                                TextAreaAction::Changed | TextAreaAction::None => {}
                            }
                        }
                    }
                    Focus::Transcript => {
                        if let Some(ev) = to_input_event(key) {
                            if let SelectionAction::CopyRequested(s) =
                                transcript.handle_event_action(ev)
                            {
                                *copied = Some(s);
                            }
                        }
                    }
                    Focus::Diff => {
                        if let Some(ev) = to_input_event(key) {
                            if let SelectionAction::CopyRequested(s) = diff.handle_event_action(ev)
                            {
                                *copied = Some(s);
                            }
                        }
                    }
                }
            }
            Event::Mouse(m) => {
                let Some(ev) = to_mouse_event(m) else {
                    continue;
                };
                if point_in(layout.transcript, ev.x, ev.y) {
                    let _ = transcript
                        .handle_event_action_in_area(layout.transcript, InputEvent::Mouse(ev));
                } else if point_in(layout.diff, ev.x, ev.y) {
                    let _ = diff.handle_event_action_in_area(layout.diff, InputEvent::Mouse(ev));
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct LayoutState {
    transcript: ratatui::layout::Rect,
    diff: ratatui::layout::Rect,
}

fn ui(
    f: &mut ratatui::Frame<'_>,
    theme: &Theme,
    transcript: &mut TranscriptView,
    diff: &mut DiffView,
    input: &mut TextArea,
    focus: Focus,
    copied: &Option<String>,
    layout: &mut LayoutState,
) -> Option<(u16, u16)> {
    let area = f.area();
    let [top, input_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(5)])
        .areas(area);

    let [left, right] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .areas(top);

    let left_title = match focus {
        Focus::Transcript => "Transcript (focused)".cyan().bold(),
        _ => "Transcript".cyan(),
    };
    let right_title = match focus {
        Focus::Diff => "Diff (focused)".cyan().bold(),
        _ => "Diff".cyan(),
    };

    let left_block = Block::default().title(left_title).borders(Borders::ALL);
    let right_block = Block::default().title(right_title).borders(Borders::ALL);

    let left_inner = left_block.inner(left);
    let right_inner = right_block.inner(right);
    layout.transcript = left_inner;
    layout.diff = right_inner;
    f.render_widget(left_block, left);
    f.render_widget(right_block, right);

    let mut input_title = match focus {
        Focus::Input => "Input (focused)".to_string(),
        _ => "Input".to_string(),
    };
    if let Some(s) = copied {
        input_title.push_str(&format!(" (copied {} chars)", s.len()));
    }
    let input_title = match focus {
        Focus::Input => input_title.cyan().bold(),
        _ => input_title.cyan(),
    };
    let input_block = Block::default().title(input_title).borders(Borders::ALL);
    let input_inner = input_block.inner(input_area);
    f.render_widget(input_block, input_area);

    let buf = f.buffer_mut();
    transcript.render_ref(left_inner, buf, theme);
    diff.render_ref(right_inner, buf, theme);
    input.render_ref(input_inner, buf);

    if focus == Focus::Input {
        input.cursor_pos(input_inner)
    } else {
        None
    }
}

fn to_input_event(key: crossterm::event::KeyEvent) -> Option<InputEvent> {
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
    Some(InputEvent::Key(
        KE::new(code).with_modifiers(KM {
            shift: key
                .modifiers
                .contains(crossterm::event::KeyModifiers::SHIFT),
            ctrl: key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL),
            alt: key.modifiers.contains(crossterm::event::KeyModifiers::ALT),
        }),
    ))
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
