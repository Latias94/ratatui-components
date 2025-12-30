use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
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
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use ratatui_components::code_render::CodeRenderOptions;
use ratatui_components::code_render::CodeRenderStyles;
use ratatui_components::code_render::render_code_lines;
use ratatui_components::markdown::document::MarkdownDocument;
use ratatui_components::markdown::document::MarkdownRenderOptions;
use ratatui_components::theme::Theme;
use std::io;
use std::sync::Arc;
use std::time::Duration;

const SAMPLE_MARKDOWN: &str = r#"
# Render Core Demo

This example demonstrates using the **render core** APIs without `MarkdownView` / `CodeView`.

- Left pane: `MarkdownDocument::render(...)`
- Right pane: `render_code_lines(...)`

Keys:

- `j/k` or `↑/↓`: scroll focused pane
- `Tab`: switch focus
- `q`: quit

```rs
fn main() {
    println!("hello from markdown");
}
```
"#;

const SAMPLE_CODE: &[&str] = &[
    "diff --git a/main.rs b/main.rs",
    "--- a/main.rs",
    "+++ b/main.rs",
    "@@ -1,3 +1,3 @@",
    " fn main() {",
    "-    println!(\"old\");",
    "+    println!(\"new\");",
    " }",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
    Markdown,
    Code,
}

struct App {
    focus: Focus,
    md_scroll: (u16, u16),
    code_scroll: (u16, u16),
    cached_md_width: Option<u16>,
    cached_code_width: Option<u16>,
    md_text: ratatui::text::Text<'static>,
    code_text: ratatui::text::Text<'static>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            focus: Focus::Markdown,
            md_scroll: (0, 0),
            code_scroll: (0, 0),
            cached_md_width: None,
            cached_code_width: None,
            md_text: ratatui::text::Text::default(),
            code_text: ratatui::text::Text::default(),
        }
    }
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run(&mut terminal);

    disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    res
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let theme = Theme::default();
    let mut app = App::default();

    let md_opts = MarkdownRenderOptions::default();
    let md_doc = MarkdownDocument::parse(SAMPLE_MARKDOWN, &md_opts);

    #[cfg(feature = "syntect")]
    let highlighter: Option<Arc<dyn ratatui_components::text::CodeHighlighter + Send + Sync>> = {
        use ratatui_components::syntax::syntect::SyntectHighlighter;
        Some(Arc::new(SyntectHighlighter::new()))
    };
    #[cfg(not(feature = "syntect"))]
    let highlighter: Option<Arc<dyn ratatui_components::text::CodeHighlighter + Send + Sync>> =
        None;

    loop {
        terminal.draw(|f| {
            let area = f.area();
            let [left, right] = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                .areas(area);

            let md_inner_w = left.width.saturating_sub(2);
            if app.cached_md_width != Some(md_inner_w) {
                let rendered =
                    md_doc.render(md_inner_w, &theme, &md_opts, highlighter.as_ref().cloned());
                app.md_text = rendered.into_text();
                app.cached_md_width = Some(md_inner_w);
            }

            let code_inner_w = right.width.saturating_sub(2);
            if app.cached_code_width != Some(code_inner_w) {
                let hi_ref = highlighter
                    .as_ref()
                    .map(|h| h.as_ref() as &dyn ratatui_components::text::CodeHighlighter);
                let rendered = render_code_lines(
                    SAMPLE_CODE,
                    Some("diff"),
                    hi_ref,
                    CodeRenderStyles {
                        base: theme.code_inline,
                        gutter: theme.text_muted,
                    },
                    CodeRenderOptions {
                        show_line_numbers: true,
                        ..Default::default()
                    },
                );
                app.code_text = rendered.into_text();
                app.cached_code_width = Some(code_inner_w);
            }

            let md_border = if app.focus == Focus::Markdown {
                theme
                    .text_primary
                    .add_modifier(ratatui::style::Modifier::BOLD)
            } else {
                theme.text_muted
            };
            let code_border = if app.focus == Focus::Code {
                theme
                    .text_primary
                    .add_modifier(ratatui::style::Modifier::BOLD)
            } else {
                theme.text_muted
            };

            let md = Paragraph::new(app.md_text.clone())
                .block(
                    Block::default()
                        .title("MarkdownDocument (render core)")
                        .borders(Borders::ALL)
                        .border_style(md_border),
                )
                .wrap(Wrap { trim: false })
                .scroll(app.md_scroll);
            f.render_widget(md, left);

            let code = Paragraph::new(app.code_text.clone())
                .block(
                    Block::default()
                        .title("render_code_lines (render core)")
                        .borders(Borders::ALL)
                        .border_style(code_border),
                )
                .style(Style::default())
                .wrap(Wrap { trim: false })
                .scroll(app.code_scroll);
            f.render_widget(code, right);
        })?;

        if !crossterm::event::poll(Duration::from_millis(50))? {
            continue;
        }

        match crossterm::event::read()? {
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if key.code == KeyCode::Char('q') {
                    return Ok(());
                }
                if key.code == KeyCode::Tab {
                    app.focus = match app.focus {
                        Focus::Markdown => Focus::Code,
                        Focus::Code => Focus::Markdown,
                    };
                    continue;
                }

                let delta = match (key.modifiers, key.code) {
                    (_, KeyCode::Up) | (_, KeyCode::Char('k')) => -1i16,
                    (_, KeyCode::Down) | (_, KeyCode::Char('j')) => 1i16,
                    (KeyModifiers::CONTROL, KeyCode::Char('u')) => -10i16,
                    (KeyModifiers::CONTROL, KeyCode::Char('d')) => 10i16,
                    _ => 0i16,
                };
                if delta == 0 {
                    continue;
                }

                let scroll = match app.focus {
                    Focus::Markdown => &mut app.md_scroll,
                    Focus::Code => &mut app.code_scroll,
                };
                let y = scroll.0 as i16;
                let y = (y + delta).max(0) as u16;
                *scroll = (y, scroll.1);
            }
            _ => {}
        }
    }
}
