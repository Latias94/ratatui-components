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
use ratatui::layout::Rect;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui_components::datagrid::view::DataGridAction;
use ratatui_components::datagrid::view::DataGridColumn;
use ratatui_components::datagrid::view::DataGridView;
use ratatui_components::datagrid::view::DataGridViewOptions;
use ratatui_components::input::InputEvent;
use ratatui_components::render;
use ratatui_components::theme::Theme;
use std::io;
use std::time::Duration;

fn main() -> io::Result<()> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    crossterm::execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let theme = Theme::default();

    let columns: Vec<DataGridColumn> = (0..200)
        .map(|i| DataGridColumn::new(format!("col_{i:03}"), 12))
        .collect();

    let mut grid = DataGridView::with_options(DataGridViewOptions {
        multi_select: true,
        selection_follows_cursor: true,
        ..Default::default()
    });
    grid.set_columns(columns);
    grid.set_row_count(200_000);

    let res = run(&mut terminal, &theme, &mut grid);

    disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
}

fn run<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    theme: &Theme,
    grid: &mut DataGridView,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| {
            let area = f.area();
            let block = Block::default()
                .title("DataGridView (hjkl/←→↑↓, PgUp/PgDn, g/G, Space/Shift+arrows, Enter, q)")
                .borders(Borders::ALL);
            let inner = block.inner(area);
            f.render_widget(block, area);

            let buf = f.buffer_mut();
            let grid_area = Rect::new(
                inner.x,
                inner.y,
                inner.width,
                inner.height.saturating_sub(1),
            );
            let status_area = Rect::new(inner.x, inner.y + grid_area.height, inner.width, 1);

            grid.render(grid_area, buf, theme, |cell_area, ctx, buf, theme| {
                if cell_area.width == 0 || cell_area.height == 0 {
                    return;
                }

                let text = format!("r{} c{}", ctx.cell.row, ctx.cell.col);
                let clipped = render::slice_by_cols(&text, ctx.clip_left, cell_area.width);
                buf.set_stringn(
                    cell_area.x,
                    cell_area.y,
                    clipped,
                    cell_area.width as usize,
                    theme.text_primary,
                );
            });

            render_status(status_area, buf, theme, grid);
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
                match grid.handle_event(ev) {
                    DataGridAction::Activated(cell) => {
                        eprintln!("Activated cell: r{} c{}", cell.row, cell.col);
                    }
                    DataGridAction::Redraw
                    | DataGridAction::SelectionChanged
                    | DataGridAction::None => {}
                }
            }
        }
    }
}

fn render_status(
    area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    theme: &Theme,
    grid: &DataGridView,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let cursor = grid
        .cursor()
        .map(|c| format!("r{} c{}", c.row, c.col))
        .unwrap_or("-".to_string());
    let sel = match grid.selection() {
        ratatui_components::datagrid::view::Selection::None => "-".to_string(),
        ratatui_components::datagrid::view::Selection::Single(c) => {
            format!("r{} c{}", c.row, c.col)
        }
        ratatui_components::datagrid::view::Selection::Rect { start, end } => {
            format!("r{}c{}..r{}c{}", start.row, start.col, end.row, end.col)
        }
    };
    let pct = grid.state.percent_y().unwrap_or(0);
    let s = format!("cursor={cursor}  selection={sel}  scroll={pct}%");
    let span = Span::styled(s, theme.text_muted);
    buf.set_span(area.x, area.y, &span, area.width);
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
