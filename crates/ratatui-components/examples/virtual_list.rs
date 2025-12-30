use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui_components::input::InputEvent;
use ratatui_components::theme::Theme;
use ratatui_components::virtual_list::VirtualListAction;
use ratatui_components::virtual_list::VirtualListView;
use ratatui_components::virtual_list::VirtualListViewOptions;
use std::io;
use std::time::Duration;

fn main() -> io::Result<()> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    crossterm::execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let theme = Theme::default();
    let items: Vec<String> = (0..200_000)
        .map(|i| format!("{i:06}  The quick brown fox jumps over the lazy dog"))
        .collect();

    let mut list = VirtualListView::with_options(VirtualListViewOptions {
        multi_select: true,
        selection_follows_cursor: false,
        ..Default::default()
    });
    list.set_fixed_item_size(1);
    list.set_cursor(Some(0), items.len());

    let res = run(&mut terminal, &theme, &items, &mut list);

    disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
}

fn run<B: ratatui::backend::Backend<Error = io::Error>>(
    terminal: &mut Terminal<B>,
    theme: &Theme,
    items: &[String],
    list: &mut VirtualListView,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| {
            let area = f.area();
            let [main, status] = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .areas(area);

            let block = Block::default()
                .title("VirtualListView (j/k, ↑/↓, PgUp/PgDn, g/G, Space, Enter, q)")
                .borders(Borders::ALL);
            let inner = block.inner(main);
            f.render_widget(block, main);

            let buf = f.buffer_mut();
            list.render(
                inner,
                buf,
                theme,
                items.len(),
                |item_area, ctx, buf, theme| {
                    if item_area.height == 0 {
                        return None;
                    }
                    let s = items.get(ctx.index).map(String::as_str).unwrap_or("");
                    let prefix = if ctx.is_selected { "[x] " } else { "[ ] " };
                    let line = format!("{prefix}{s}");
                    buf.set_stringn(
                        item_area.x,
                        item_area.y,
                        line,
                        item_area.width as usize,
                        theme.text_primary,
                    );
                    None
                },
            );

            let cursor = list.cursor().unwrap_or(0);
            let pct = list.viewport.percent_y().unwrap_or(0);
            let sel = list.selected().len();
            let status_line = format!("cursor={cursor}  selected={sel}  scroll={pct}%");
            let status_span = Span::styled(status_line, Style::default());
            buf.set_span(status.x, status.y, &status_span, status.width);
        })?;

        if crossterm::event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = crossterm::event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            if matches!(key.code, KeyCode::Char('q')) {
                return Ok(());
            }

            if let Some(ev) = to_input_event(key) {
                match list.handle_event(ev, items.len()) {
                    VirtualListAction::Activated(idx) => {
                        if let Some(s) = items.get(idx) {
                            eprintln!("Activated: {s}");
                        }
                    }
                    VirtualListAction::Redraw
                    | VirtualListAction::SelectionChanged
                    | VirtualListAction::None => {}
                }
            }
        }
    }
}

fn to_input_event(key: crossterm::event::KeyEvent) -> Option<InputEvent> {
    let key = to_key_event(key)?;
    Some(InputEvent::Key(key))
}

fn to_key_event(key: crossterm::event::KeyEvent) -> Option<ratatui_components::input::KeyEvent> {
    use ratatui_components::input::KeyCode as K;
    use ratatui_components::input::KeyEvent as E;
    use ratatui_components::input::KeyModifiers as M;

    let modifiers = M {
        shift: key.modifiers.contains(KeyModifiers::SHIFT),
        ctrl: key.modifiers.contains(KeyModifiers::CONTROL),
        alt: key.modifiers.contains(KeyModifiers::ALT),
    };

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

    Some(E { code, modifiers })
}
