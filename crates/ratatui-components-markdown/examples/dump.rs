use ratatui::text::Line;
use ratatui_components::theme::Theme;
use ratatui_components_markdown::view::{MarkdownView, MarkdownViewOptions, TableStyle};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

fn main() -> io::Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return Ok(());
    }

    let mut width: u16 = 80;
    let mut show_link_destinations = false;
    let mut base_url: Option<String> = None;
    let mut padding_left: u16 = 0;
    let mut padding_right: u16 = 0;
    let mut table_style = TableStyle::Glow;
    let mut footnotes_at_end = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--width" => {
                width = parse_u16(&args, &mut i, "--width")?;
            }
            "--show-link-destinations" => {
                show_link_destinations = true;
                i += 1;
            }
            "--base-url" => {
                base_url = Some(parse_string(&args, &mut i, "--base-url")?);
            }
            "--padding-left" => {
                padding_left = parse_u16(&args, &mut i, "--padding-left")?;
            }
            "--padding-right" => {
                padding_right = parse_u16(&args, &mut i, "--padding-right")?;
            }
            "--table-style" => {
                let v = parse_string(&args, &mut i, "--table-style")?;
                table_style = match v.as_str() {
                    "glow" => TableStyle::Glow,
                    "box" => TableStyle::Box,
                    other => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("unknown table style: {other}"),
                        ));
                    }
                };
            }
            "--footnotes-at-end" => {
                footnotes_at_end = true;
                i += 1;
            }
            _ => break,
        }
    }

    let input = if i < args.len() {
        let path = &args[i];
        let input = fs::read_to_string(path)?;
        if base_url.is_none()
            && let Some(parent) = Path::new(path).parent()
        {
            let abs = fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
            base_url = Some(format!("{}/", abs.display()));
        }
        input
    } else {
        let mut s = String::new();
        io::stdin().read_to_string(&mut s)?;
        s
    };

    let mut view = MarkdownView::with_options(MarkdownViewOptions {
        show_link_destinations,
        base_url,
        padding_left,
        padding_right,
        table_style,
        footnotes_at_end,
        ..MarkdownViewOptions::default()
    });
    view.set_markdown(&input);

    let theme = Theme::default();
    for line in view.lines_for_width(width, &theme) {
        println!("{}", line_to_plain(&line));
    }

    Ok(())
}

fn print_help() {
    eprintln!(
        "Usage: dump [options] [path]\n\
\n\
Options:\n\
  --width <n>                 Wrap width (default: 80)\n\
  --show-link-destinations    Show (url) after link text\n\
  --base-url <url>            Resolve relative links/images against this base\n\
  --padding-left <n>          Left padding (columns)\n\
  --padding-right <n>         Right padding (columns)\n\
  --table-style <glow|box>    Table rendering style (default: glow)\n\
  --footnotes-at-end          Collect footnote definitions at the end\n\
  -h, --help                  Show this help\n\
\n\
If [path] is omitted, reads Markdown from stdin."
    );
}

fn parse_u16(args: &[String], i: &mut usize, flag: &str) -> io::Result<u16> {
    let Some(v) = args.get(*i + 1) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{flag} expects a value"),
        ));
    };
    *i += 2;
    v.parse::<u16>().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{flag} invalid u16: {e}"),
        )
    })
}

fn parse_string(args: &[String], i: &mut usize, flag: &str) -> io::Result<String> {
    let Some(v) = args.get(*i + 1) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{flag} expects a value"),
        ));
    };
    *i += 2;
    Ok(v.to_string())
}

fn line_to_plain(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect::<Vec<_>>()
        .join("")
}
