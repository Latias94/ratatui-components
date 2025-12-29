use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEventKind;
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
use ratatui_components::input::InputEvent;
use ratatui_components::input::KeyCode as K;
use ratatui_components::input::KeyEvent as KE;
use ratatui_components::input::KeyModifiers as KM;
use ratatui_components::textarea::TextArea;
use ratatui_components::textarea::TextAreaAction;
use ratatui_components::theme::Theme;
use ratatui_components_syntax_syntect::SyntectHighlighter;
use ratatui_components_transcript::view::Role;
use ratatui_components_transcript::view::TranscriptView;
use std::io;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
    Transcript,
    Input,
}

struct StreamState {
    full: String,
    pos: usize, // byte index
    next_emit: Instant,
}

fn main() -> io::Result<()> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    crossterm::execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let theme = Theme::default();
    let highlighter = Arc::new(SyntectHighlighter::new());

    let mut transcript = TranscriptView::new();
    transcript.set_highlighter(Some(highlighter));
    transcript.push_markdown(
        Role::System,
        "# TranscriptView demo\n\nType below and press Enter to submit.",
    );
    transcript.push_diff(
        Role::Tool,
        "diff --git a/a.txt b/a.txt\n@@ -1 +1 @@\n-hello\n+hello world\n",
    );

    let mut input = TextArea::new();
    let mut focus = Focus::Input;

    let res = run(
        &mut terminal,
        &theme,
        &mut transcript,
        &mut input,
        &mut focus,
    );

    disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
}

fn run<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    theme: &Theme,
    transcript: &mut TranscriptView,
    input: &mut TextArea,
    focus: &mut Focus,
) -> io::Result<()> {
    let mut stream: Option<StreamState> = None;

    loop {
        terminal.draw(|f| {
            let area = f.area();
            let [top, bottom] = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(5)])
                .areas(area);

            let top_block = Block::default()
                .title(match *focus {
                    Focus::Transcript => "Transcript (focused)".cyan().bold(),
                    Focus::Input => "Transcript".cyan(),
                })
                .borders(Borders::ALL);
            let bottom_block = Block::default()
                .title(match *focus {
                    Focus::Input => "Input (focused)".cyan().bold(),
                    Focus::Transcript => "Input".cyan(),
                })
                .borders(Borders::ALL);

            let top_inner = top_block.inner(top);
            let bottom_inner = bottom_block.inner(bottom);
            f.render_widget(top_block, top);
            f.render_widget(bottom_block, bottom);

            let buf = f.buffer_mut();
            transcript.render_ref(top_inner, buf, theme);
            input.render_ref(bottom_inner, buf);

            if *focus == Focus::Input
                && let Some((x, y)) = input.cursor_pos(bottom_inner)
            {
                f.set_cursor_position((x, y));
            }
        })?;

        if let Some(s) = stream.as_mut() {
            let now = Instant::now();
            if now >= s.next_emit {
                if s.pos < s.full.len() {
                    let end = next_chunk_boundary(&s.full, s.pos, 12);
                    let chunk = &s.full[s.pos..end];
                    let _ = transcript.append_to_last_markdown(Role::Assistant, chunk);
                    s.pos = end;
                    s.next_emit = now + Duration::from_millis(20);
                } else {
                    stream = None;
                }
            }
        }

        if crossterm::event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = crossterm::event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            if matches!(key.code, KeyCode::Char('q')) {
                return Ok(());
            }
            if matches!(key.code, KeyCode::Tab) {
                *focus = match *focus {
                    Focus::Transcript => Focus::Input,
                    Focus::Input => Focus::Transcript,
                };
                continue;
            }

            if let Some(ev) = to_input_event(key) {
                match *focus {
                    Focus::Transcript => {
                        let _ = transcript.handle_event(ev);
                    }
                    Focus::Input => match input.input(ev) {
                        TextAreaAction::Submitted(text) => {
                            transcript.push_markdown(Role::User, &text);
                            transcript.push_markdown(Role::Assistant, "");
                            stream = Some(StreamState {
                                full: format!(
                                    "收到。下面是一个流式渲染示例：\n\n- [x] task list item\n- [ ] task list item\n\n```txt\n{text}\n```\n"
                                ),
                                pos: 0,
                                next_emit: Instant::now(),
                            });
                        }
                        TextAreaAction::Changed | TextAreaAction::None => {}
                    },
                }
            }
        }
    }
}

fn to_input_event(key: crossterm::event::KeyEvent) -> Option<InputEvent> {
    let modifiers = KM {
        shift: key
            .modifiers
            .contains(crossterm::event::KeyModifiers::SHIFT),
        ctrl: key
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL),
        alt: key.modifiers.contains(crossterm::event::KeyModifiers::ALT),
    };

    let code = match key.code {
        KeyCode::Char(c) => K::Char(c),
        KeyCode::Enter => K::Enter,
        KeyCode::Backspace => K::Backspace,
        KeyCode::Delete => K::Delete,
        KeyCode::Left => K::Left,
        KeyCode::Right => K::Right,
        KeyCode::Up => K::Up,
        KeyCode::Down => K::Down,
        KeyCode::Home => K::Home,
        KeyCode::End => K::End,
        KeyCode::PageUp => K::PageUp,
        KeyCode::PageDown => K::PageDown,
        KeyCode::Tab => K::Tab,
        KeyCode::Esc => K::Esc,
        _ => return None,
    };

    Some(InputEvent::Key(KE { code, modifiers }))
}

fn next_chunk_boundary(s: &str, start: usize, max_chars: usize) -> usize {
    if start >= s.len() {
        return s.len();
    }
    let mut it = s[start..].char_indices();
    for _ in 0..max_chars {
        if it.next().is_none() {
            return s.len();
        }
    }
    match it.next() {
        Some((i, _)) => start + i,
        None => s.len(),
    }
}
