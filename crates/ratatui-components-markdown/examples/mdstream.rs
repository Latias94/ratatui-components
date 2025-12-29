#[cfg(not(feature = "mdstream"))]
fn main() {
    eprintln!(
        "This example requires the `mdstream` feature.\n\
Run:\n  cargo run -p ratatui-components-markdown --features mdstream --example mdstream"
    );
}

#[cfg(feature = "mdstream")]
mod with_mdstream {
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
    use ratatui::text::Line;
    use ratatui::text::Span;
    use ratatui::widgets::Block;
    use ratatui::widgets::Borders;
    use ratatui::widgets::Paragraph;
    use ratatui_components::theme::Theme;
    use ratatui_components_markdown::streaming::MarkdownStreamView;
    use ratatui_components_markdown::view::MarkdownView;
    use ratatui_components_markdown::view::MarkdownViewOptions;
    use ratatui_components_syntax_syntect::SyntectHighlighter;
    use std::io;
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;
    use std::time::Instant;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum CoalesceMode {
        Newline,
        Balanced,
        TimeOnly,
    }

    impl CoalesceMode {
        fn label(self) -> &'static str {
            match self {
                Self::Newline => "newline",
                Self::Balanced => "balanced",
                Self::TimeOnly => "time-only",
            }
        }

        fn next(self) -> Self {
            match self {
                Self::Newline => Self::Balanced,
                Self::Balanced => Self::TimeOnly,
                Self::TimeOnly => Self::Newline,
            }
        }

        fn flush_on_newline(self) -> bool {
            match self {
                Self::Newline | Self::Balanced => true,
                Self::TimeOnly => false,
            }
        }

        fn max_delay(self) -> Duration {
            match self {
                Self::Newline => Duration::from_millis(120),
                Self::Balanced => Duration::from_millis(30),
                Self::TimeOnly => Duration::from_millis(60),
            }
        }

        fn max_bytes(self) -> usize {
            match self {
                Self::Newline => 8 * 1024,
                Self::Balanced => 4 * 1024,
                Self::TimeOnly => 4 * 1024,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum FlushReason {
        Newline,
        Timer,
        MaxBytes,
        Finalize,
    }

    struct App {
        raw: String,
        raw_view: MarkdownView,
        stream_view: MarkdownStreamView,
        pending: String,
        coalesce: CoalesceMode,
        follow_tail: bool,
        done: bool,
        last_flush: Instant,
        last_reason: Option<FlushReason>,
        last_flushed_bytes: usize,
        in_deltas: u64,
        flushes: u64,
    }

    pub fn main() -> io::Result<()> {
        if std::env::args().any(|a| a == "-h" || a == "--help") {
            eprintln!(
                "Usage: cargo run -p ratatui-components-markdown --features mdstream --example mdstream"
            );
            return Ok(());
        }

        let mut stdout = io::stdout();
        enable_raw_mode()?;
        crossterm::execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let theme = Theme::default();
        let highlighter = Arc::new(SyntectHighlighter::new());

        let mut raw_view = MarkdownView::with_options(MarkdownViewOptions {
            padding_left: 1,
            padding_right: 1,
            show_code_line_numbers: true,
            ..Default::default()
        });
        raw_view.set_highlighter(Some(highlighter.clone()));

        let mut stream_view = MarkdownStreamView::with_options(
            mdstream::Options::default(),
            MarkdownViewOptions {
                padding_left: 1,
                padding_right: 1,
                show_code_line_numbers: true,
                ..Default::default()
            },
        );
        stream_view.set_highlighter(Some(highlighter));
        stream_view.set_pending_code_fence_max_lines(Some(40));

        let (tx, rx) = mpsc::channel::<String>();
        spawn_demo(tx);

        let mut app = App {
            raw: String::new(),
            raw_view,
            stream_view,
            pending: String::new(),
            coalesce: CoalesceMode::Balanced,
            follow_tail: true,
            done: false,
            last_flush: Instant::now(),
            last_reason: None,
            last_flushed_bytes: 0,
            in_deltas: 0,
            flushes: 0,
        };

        let res = run(&mut terminal, &theme, &mut app, rx);
        disable_raw_mode()?;
        crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        res
    }

    fn spawn_demo(tx: mpsc::Sender<String>) {
        thread::spawn(move || {
            let demo = demo_markdown();
            for chunk in chunk_by(&demo, 3) {
                if tx.send(chunk).is_err() {
                    return;
                }
                thread::sleep(Duration::from_millis(6));
            }
        });
    }

    fn run<B: ratatui::backend::Backend>(
        terminal: &mut Terminal<B>,
        theme: &Theme,
        app: &mut App,
        rx: mpsc::Receiver<String>,
    ) -> io::Result<()> {
        let mut rx = rx;
        loop {
            drain_deltas(app, &mut rx);
            maybe_flush(app, FlushReason::Timer);

            terminal.draw(|f| ui(f, theme, app))?;

            if crossterm::event::poll(Duration::from_millis(33))?
                && let Event::Key(key) = crossterm::event::read()?
            {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('f') => {
                        app.follow_tail = !app.follow_tail;
                        if app.follow_tail {
                            app.raw_view.state.to_bottom();
                            app.stream_view.viewport.to_bottom();
                        }
                    }
                    KeyCode::Char('c') => {
                        app.coalesce = app.coalesce.next();
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        app.follow_tail = false;
                        app.raw_view.scroll_y_by(1);
                        app.stream_view.viewport.scroll_y_by(1);
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        app.follow_tail = false;
                        app.raw_view.scroll_y_by(-1);
                        app.stream_view.viewport.scroll_y_by(-1);
                    }
                    KeyCode::PageDown => {
                        app.follow_tail = false;
                        app.raw_view.state.page_down();
                        app.stream_view.viewport.page_down();
                    }
                    KeyCode::PageUp => {
                        app.follow_tail = false;
                        app.raw_view.state.page_up();
                        app.stream_view.viewport.page_up();
                    }
                    KeyCode::Char('g') | KeyCode::Home => {
                        app.follow_tail = false;
                        app.raw_view.state.to_top();
                        app.stream_view.viewport.to_top();
                    }
                    KeyCode::Char('G') | KeyCode::End => {
                        app.follow_tail = true;
                        app.raw_view.state.to_bottom();
                        app.stream_view.viewport.to_bottom();
                    }
                    _ => {}
                }
            }
        }
    }

    fn drain_deltas(app: &mut App, rx: &mut mpsc::Receiver<String>) {
        loop {
            match rx.try_recv() {
                Ok(delta) => {
                    app.pending.push_str(&delta);
                    app.in_deltas = app.in_deltas.saturating_add(1);

                    if app.coalesce.flush_on_newline() && delta.contains('\n') {
                        maybe_flush(app, FlushReason::Newline);
                    } else if app.pending.len() >= app.coalesce.max_bytes() {
                        maybe_flush(app, FlushReason::MaxBytes);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if !app.done {
                        maybe_flush(app, FlushReason::Finalize);
                        let _ = app.stream_view.finalize();
                        if app.follow_tail {
                            app.stream_view.viewport.to_bottom();
                        }
                        app.done = true;
                    }
                    break;
                }
            }
        }
    }

    fn maybe_flush(app: &mut App, reason: FlushReason) {
        if app.pending.is_empty() {
            return;
        }

        let now = Instant::now();
        if reason == FlushReason::Timer
            && now.duration_since(app.last_flush) < app.coalesce.max_delay()
        {
            return;
        }

        let delta = std::mem::take(&mut app.pending);
        app.last_flushed_bytes = delta.len();
        app.last_reason = Some(reason);
        app.last_flush = now;
        app.flushes = app.flushes.saturating_add(1);

        app.raw.push_str(&delta);
        app.raw_view.set_markdown(&app.raw);

        let _ = app.stream_view.append(&delta);

        if app.follow_tail {
            app.raw_view.state.to_bottom();
            app.stream_view.viewport.to_bottom();
        }
    }

    fn ui(f: &mut ratatui::Frame<'_>, theme: &Theme, app: &mut App) {
        let area = f.area();
        let [main, status_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .areas(area);

        let [left, right] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(main);

        let left_block = Block::default()
            .title("Raw (set_markdown)")
            .borders(Borders::ALL);
        let right_block = Block::default()
            .title("mdstream (pending terminator)")
            .borders(Borders::ALL);

        let inner_left = left_block.inner(left);
        let inner_right = right_block.inner(right);
        f.render_widget(left_block, left);
        f.render_widget(right_block, right);

        app.raw_view.render_ref(inner_left, f.buffer_mut(), theme);
        app.stream_view
            .render_ref(inner_right, f.buffer_mut(), theme);

        let pending_kind = app
            .stream_view
            .pending()
            .map(|b| format!("{:?}", b.kind))
            .unwrap_or("-".to_string());
        let reason = app
            .last_reason
            .map(|r| format!("{r:?}"))
            .unwrap_or("-".to_string());

        let status = format!(
            "q quit | j/k scroll | g/G top/bottom | f follow-tail={} | c coalesce={} | done={} | pending_kind={} | flush={} bytes={} | in_deltas={} flushes={}",
            app.follow_tail,
            app.coalesce.label(),
            app.done,
            pending_kind,
            reason,
            app.last_flushed_bytes,
            app.in_deltas,
            app.flushes,
        );
        let p = Paragraph::new(Line::from(vec![Span::styled(status, theme.text_muted)]));
        f.render_widget(p, status_area);
    }

    fn demo_markdown() -> String {
        let mut s = String::new();
        s.push_str("# mdstream + MarkdownView\n\n");
        s.push_str("This demo streams Markdown in tiny chunks.\n\n");
        s.push_str("- Left pane: raw string fed into `MarkdownView::set_markdown()`\n");
        s.push_str(
            "- Right pane: `MarkdownStreamView` incremental rendering (committed blocks cached)\n\n",
        );

        s.push_str("## Task List\n\n");
        s.push_str("- [x] task list item\n");
        s.push_str("- [ ] task list item\n\n");

        s.push_str("## Table\n\n");
        s.push_str("| Name | Value | Notes |\n");
        s.push_str("|:-----|------:|:------|\n");
        s.push_str("| foo  | 123   | left / right alignment |\n");
        s.push_str("| bar  | 456   | wraps when the terminal is narrow |\n\n");

        s.push_str("## Open code fence (streaming)\n\n");
        s.push_str("```rs\n");
        s.push_str("fn main() {\n");
        s.push_str("    println!(\"hello\");\n");
        s.push_str("    // pretend we are still streaming...\n");
        s.push_str("    // and the fence is not closed yet\n");
        s.push('\n');
        s.push_str("    let x = 1 + 2;\n");
        s.push_str("    println!(\"x={x}\");\n");
        s.push_str("}\n");
        s.push_str("```\n\n");

        s.push_str("## Links\n\n");
        s.push_str("A relative link: [docs](./docs/path)\n\n");
        s.push_str("A long paragraph to test wrapping. ");
        s.push_str("The quick brown fox jumps over the lazy dog. ");
        s.push_str("The quick brown fox jumps over the lazy dog. ");
        s.push_str("The quick brown fox jumps over the lazy dog.\n\n");

        s.push_str("Done.\n");
        s
    }

    fn chunk_by(s: &str, n: usize) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = String::new();
        for ch in s.chars() {
            cur.push(ch);
            if cur.chars().count() >= n {
                out.push(std::mem::take(&mut cur));
            }
        }
        if !cur.is_empty() {
            out.push(cur);
        }
        out
    }
}

#[cfg(feature = "mdstream")]
fn main() -> std::io::Result<()> {
    with_mdstream::main()
}
