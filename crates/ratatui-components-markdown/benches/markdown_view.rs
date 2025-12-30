use criterion::Criterion;
use criterion::black_box;
use criterion::criterion_group;
use criterion::criterion_main;
use ratatui_components_core::text::NoHighlight;
use ratatui_components_core::theme::Theme;
use ratatui_components_markdown::view::MarkdownView;
use ratatui_components_markdown::view::MarkdownViewOptions;
use std::sync::Arc;

fn sample_markdown(code_lines: usize) -> String {
    let mut s = String::new();
    s.push_str("# Performance\n\n");
    s.push_str("This is a long paragraph to stress wrapping. ");
    for _ in 0..12 {
        s.push_str("The quick brown fox jumps over the lazy dog. ");
    }
    s.push('\n');
    s.push('\n');

    s.push_str("## Task List\n\n");
    s.push_str("- [x] task list item\n");
    s.push_str("- [ ] task list item\n\n");

    s.push_str("## Table\n\n");
    s.push_str("| Name | Value | Notes |\n");
    s.push_str("|:-----|------:|:------|\n");
    s.push_str("| foo  | 123   | left / right alignment |\n");
    s.push_str("| bar  | 456   | wraps when the terminal is narrow |\n\n");

    s.push_str("## Code\n\n");
    s.push_str("```rs\n");
    s.push_str("fn main() {\n");
    for i in 0..code_lines {
        s.push_str(&format!("    let x{i} = {i} + 1;\n"));
    }
    s.push_str("    println!(\"done\");\n");
    s.push_str("}\n");
    s.push_str("```\n");
    s
}

fn chunks(s: &str, n_chars: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in s.chars() {
        cur.push(ch);
        if cur.chars().count() >= n_chars {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn bench_set_markdown_no_highlight(c: &mut Criterion) {
    let theme = Theme::default();
    let md = sample_markdown(200);
    let hi = Arc::new(NoHighlight);
    c.bench_function("markdown_view/set_markdown+layout/no_highlight", |b| {
        b.iter(|| {
            let mut view = MarkdownView::with_options(MarkdownViewOptions {
                show_code_line_numbers: true,
                ..Default::default()
            });
            view.set_highlighter(Some(hi.clone()));
            view.set_markdown(black_box(&md));
            let lines = view.lines_for_width(black_box(96), &theme);
            black_box(lines.len());
        })
    });
}

fn bench_set_markdown_syntect(c: &mut Criterion) {
    let theme = Theme::default();
    let md = sample_markdown(200);
    let hi = Arc::new(NoHighlight);
    c.bench_function("markdown_view/set_markdown+layout/no_highlight_2", |b| {
        b.iter(|| {
            let mut view = MarkdownView::with_options(MarkdownViewOptions {
                show_code_line_numbers: true,
                ..Default::default()
            });
            view.set_highlighter(Some(hi.clone()));
            view.set_markdown(black_box(&md));
            let lines = view.lines_for_width(black_box(96), &theme);
            black_box(lines.len());
        })
    });
}

fn bench_streaming_raw_newline_flush_syntect(c: &mut Criterion) {
    let theme = Theme::default();
    let md = sample_markdown(200);
    let deltas = chunks(&md, 3);
    let hi = Arc::new(NoHighlight);

    c.bench_function(
        "markdown_view/streaming/raw/newline_flush/no_highlight",
        |b| {
            b.iter(|| {
                let mut view = MarkdownView::with_options(MarkdownViewOptions {
                    show_code_line_numbers: true,
                    ..Default::default()
                });
                view.set_highlighter(Some(hi.clone()));

                let mut raw = String::new();
                for d in &deltas {
                    raw.push_str(d);
                    if d.contains('\n') {
                        view.set_markdown(&raw);
                        let _ = view.lines_for_width(96, &theme);
                    }
                }

                black_box(raw.len());
            })
        },
    );
}

criterion_group!(
    benches,
    bench_set_markdown_no_highlight,
    bench_set_markdown_syntect,
    bench_streaming_raw_newline_flush_syntect
);
criterion_main!(benches);
