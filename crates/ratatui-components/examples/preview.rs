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
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui_components::help::HelpBar;
use ratatui_components::help::HelpBarOptions;
use ratatui_components::keymap;
use ratatui_components::keymap::Binding;
use ratatui_components::textarea::TextArea;
use ratatui_components::textarea::TextAreaAction;
use ratatui_components::theme::Theme;
use ratatui_components_diff::DiffView;
use ratatui_components_markdown::view::MarkdownView;
use ratatui_components_syntax_syntect::SyntectHighlighter;
use std::io;
use std::sync::Arc;
use std::time::Duration;

const SAMPLE_MARKDOWN: &str = r#"
# ratatui-components

This is a preview of **MarkdownView** and **DiffView**.

This is a [link](https://github.com/frankorz) and a footnote reference[^1].
This is a relative [link](./relative/path) (resolved via `base_url`).

## Keybindings

- `Tab`: switch focus
- `j/k` or `↑/↓`: scroll
- `h/l` or `←/→`: horizontal scroll
- `g/G`: top/bottom
- `q`: quit

## GFM Features

- [x] task list item
- [ ] task list item

| Name | Value | Notes |
|:-----|------:|:------|
| foo  | 123   | left/center/right alignment |
| bar  | 456   | wraps when the terminal is narrow |

![Glow](https://github.com/charmbracelet/glow)

> Markdown paragraphs are word-wrapped.
> Code blocks are *not* soft-wrapped by default.

```rs
fn main() {
    println!("hello");
}
```

[^1]: Footnotes render as definitions with an indented prefix.
"#;

const SAMPLE_DIFF: &str = r#"
diff --git a/main.rs b/main.rs
index 0000000..1111111 100644
--- a/main.rs
+++ b/main.rs
@@ -1,3 +1,6 @@
 fn main() {
-    println!("Hello, world!");
+    println!("Hello, ratatui-components!");
+    println!("DiffView supports scrolling.");
 }
+// end
"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
    Markdown,
    Diff,
    Input,
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
    crossterm::execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let theme = Theme::default();
    let highlighter = Arc::new(SyntectHighlighter::new());

    let mut md =
        MarkdownView::with_options(ratatui_components_markdown::view::MarkdownViewOptions {
            padding_left: 1,
            padding_right: 1,
            show_link_destinations: true,
            base_url: Some("https://example.com/docs/".to_string()),
            ..Default::default()
        });
    md.set_markdown(SAMPLE_MARKDOWN.trim());
    md.set_highlighter(Some(highlighter.clone()));

    let mut diff = DiffView::new();
    diff.set_diff(SAMPLE_DIFF.trim());
    diff.set_highlighter(Some(highlighter));

    let mut input = TextArea::new();
    let mut focus = Focus::Markdown;

    let keymap = build_keymap();
    let help = build_help_bar(&keymap);
    let res = run(
        &mut terminal,
        &theme,
        &help,
        &keymap,
        &mut md,
        &mut diff,
        &mut input,
        &mut focus,
    );

    disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
}

#[allow(clippy::too_many_arguments)]
fn run<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    theme: &Theme,
    help: &HelpBar,
    keymap: &[BindingAction],
    md: &mut MarkdownView,
    diff: &mut DiffView,
    input: &mut TextArea,
    focus: &mut Focus,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| {
            let cursor = ui(f, theme, help, md, diff, input, *focus);
            if let Some((x, y)) = cursor {
                f.set_cursor_position((x, y));
            }
        })?;

        if crossterm::event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = crossterm::event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            if let Some(action) = map_action(keymap, &key)
                && handle_action(action, focus, md, diff, input)
            {
                return Ok(());
            }

            if *focus == Focus::Input
                && let Some(ev) = to_input_event(key)
            {
                match input.input(ev) {
                    TextAreaAction::Submitted(text) => {
                        md.set_markdown(&format!(
                            "{SAMPLE_MARKDOWN}\n\n## Submitted\n\n```text\n{text}\n```\n"
                        ));
                    }
                    TextAreaAction::Changed | TextAreaAction::None => {}
                }
            }
        }
    }
}

fn ui(
    f: &mut ratatui::Frame<'_>,
    theme: &Theme,
    help: &HelpBar,
    md: &mut MarkdownView,
    diff: &mut DiffView,
    input: &mut TextArea,
    focus: Focus,
) -> Option<(u16, u16)> {
    let area = f.area();
    let [top, input_area, status] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(5),
            Constraint::Length(1),
        ])
        .areas(area);

    let [left, right] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .areas(top);

    let left_title = match focus {
        Focus::Markdown => "Markdown (focused)".cyan().bold(),
        Focus::Diff => "Markdown".cyan(),
        Focus::Input => "Markdown".cyan(),
    };
    let right_title = match focus {
        Focus::Diff => "Diff (focused)".cyan().bold(),
        Focus::Markdown => "Diff".cyan(),
        Focus::Input => "Diff".cyan(),
    };

    let left_block = Block::default().title(left_title).borders(Borders::ALL);
    let right_block = Block::default().title(right_title).borders(Borders::ALL);

    let left_inner = left_block.inner(left);
    let right_inner = right_block.inner(right);

    f.render_widget(left_block, left);
    f.render_widget(right_block, right);

    let input_title = match focus {
        Focus::Input => "Input (focused)".cyan().bold(),
        _ => "Input".cyan(),
    };
    let input_block = Block::default().title(input_title).borders(Borders::ALL);
    let input_inner = input_block.inner(input_area);
    f.render_widget(input_block, input_area);

    let buf = f.buffer_mut();
    md.render_ref(left_inner, buf, theme);
    diff.render_ref(right_inner, buf, theme);
    input.render_ref(input_inner, buf);

    help.render_ref(status, buf);

    if focus == Focus::Input {
        input.cursor_pos(input_inner)
    } else {
        None
    }
}

fn to_input_event(
    key: crossterm::event::KeyEvent,
) -> Option<ratatui_components::input::InputEvent> {
    use ratatui_components::input::InputEvent;
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

#[derive(Clone, Debug)]
struct BindingAction {
    binding: Binding,
    action: Action,
}

fn map_action(keymap: &[BindingAction], key: &crossterm::event::KeyEvent) -> Option<Action> {
    let ev = to_key_event(*key)?;
    keymap
        .iter()
        .find(|b| b.binding.matches(&ev))
        .map(|b| b.action)
}

fn handle_action(
    action: Action,
    focus: &mut Focus,
    md: &mut MarkdownView,
    diff: &mut DiffView,
    input: &mut TextArea,
) -> bool {
    match action {
        Action::Quit => return true,
        Action::FocusNext => {
            *focus = match *focus {
                Focus::Markdown => Focus::Diff,
                Focus::Diff => Focus::Input,
                Focus::Input => Focus::Markdown,
            };
        }
        Action::ScrollDown => match *focus {
            Focus::Markdown => md.scroll_y_by(1),
            Focus::Diff => diff.scroll_y_by(1),
            Focus::Input => input.state.scroll_y_by(1),
        },
        Action::ScrollUp => match *focus {
            Focus::Markdown => md.scroll_y_by(-1),
            Focus::Diff => diff.scroll_y_by(-1),
            Focus::Input => input.state.scroll_y_by(-1),
        },
        Action::ScrollLeft => match *focus {
            Focus::Markdown => md.scroll_x_by(-4),
            Focus::Diff => diff.scroll_x_by(-4),
            Focus::Input => input.state.scroll_x_by(-4),
        },
        Action::ScrollRight => match *focus {
            Focus::Markdown => md.scroll_x_by(4),
            Focus::Diff => diff.scroll_x_by(4),
            Focus::Input => input.state.scroll_x_by(4),
        },
        Action::Top => match *focus {
            Focus::Markdown => md.state.to_top(),
            Focus::Diff => diff.state.to_top(),
            Focus::Input => input.state.to_top(),
        },
        Action::Bottom => match *focus {
            Focus::Markdown => md.state.to_bottom(),
            Focus::Diff => diff.state.to_bottom(),
            Focus::Input => input.state.to_bottom(),
        },
        Action::PageDown => match *focus {
            Focus::Markdown => md.scroll_y_by(md.state.viewport_h.saturating_sub(1) as i32),
            Focus::Diff => diff.scroll_y_by(diff.state.viewport_h.saturating_sub(1) as i32),
            Focus::Input => input.state.page_down(),
        },
        Action::PageUp => match *focus {
            Focus::Markdown => md.scroll_y_by(-(md.state.viewport_h.saturating_sub(1) as i32)),
            Focus::Diff => diff.scroll_y_by(-(diff.state.viewport_h.saturating_sub(1) as i32)),
            Focus::Input => input.state.page_up(),
        },
    }
    false
}

fn build_help_bar(keymap: &[BindingAction]) -> HelpBar {
    let bindings: Vec<Binding> = keymap.iter().map(|b| b.binding.clone()).collect();

    let options = HelpBarOptions {
        style: Style::default().fg(ratatui::style::Color::DarkGray),
        key_style: Style::default()
            .fg(ratatui::style::Color::Cyan)
            .add_modifier(Modifier::BOLD),
        ..Default::default()
    };
    HelpBar::with_options(bindings, options)
}

fn build_keymap() -> Vec<BindingAction> {
    use ratatui_components::input::KeyCode as K;
    use ratatui_components::input::KeyEvent as E;
    use ratatui_components::input::KeyModifiers as M;

    vec![
        BindingAction {
            binding: Binding::new("q", "quit", vec![keymap::key_char('q')]),
            action: Action::Quit,
        },
        BindingAction {
            binding: Binding::new("Tab", "focus", vec![E::new(K::Tab)]),
            action: Action::FocusNext,
        },
        BindingAction {
            binding: Binding::new("j/↓", "down", vec![keymap::key_char('j'), E::new(K::Down)]),
            action: Action::ScrollDown,
        },
        BindingAction {
            binding: Binding::new("k/↑", "up", vec![keymap::key_char('k'), E::new(K::Up)]),
            action: Action::ScrollUp,
        },
        BindingAction {
            binding: Binding::new("h/←", "left", vec![keymap::key_char('h'), E::new(K::Left)]),
            action: Action::ScrollLeft,
        },
        BindingAction {
            binding: Binding::new(
                "l/→",
                "right",
                vec![keymap::key_char('l'), E::new(K::Right)],
            ),
            action: Action::ScrollRight,
        },
        BindingAction {
            binding: Binding::new("g", "top", vec![keymap::key_char('g')]),
            action: Action::Top,
        },
        BindingAction {
            binding: Binding::new(
                "G",
                "bottom",
                vec![E::new(K::Char('G')).with_modifiers(M {
                    shift: true,
                    ctrl: false,
                    alt: false,
                })],
            ),
            action: Action::Bottom,
        },
        BindingAction {
            binding: Binding::new(
                "PgDn/^d",
                "page down",
                vec![E::new(K::PageDown), keymap::key_ctrl('d')],
            ),
            action: Action::PageDown,
        },
        BindingAction {
            binding: Binding::new(
                "PgUp/^u",
                "page up",
                vec![E::new(K::PageUp), keymap::key_ctrl('u')],
            ),
            action: Action::PageUp,
        },
    ]
}
