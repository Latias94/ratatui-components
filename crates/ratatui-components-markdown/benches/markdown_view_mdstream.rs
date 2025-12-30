use criterion::Criterion;
use criterion::black_box;
use criterion::criterion_group;
use criterion::criterion_main;
use ratatui_components_core::text::NoHighlight;
use ratatui_components_core::theme::Theme;
use ratatui_components_markdown::streaming::MarkdownStream;
use ratatui_components_markdown::streaming::MarkdownStreamView;
use ratatui_components_markdown::view::MarkdownView;
use ratatui_components_markdown::view::MarkdownViewOptions;
use ratatui_components_syntax::syntect::SyntectHighlighter;
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

fn bench_streaming_mdstream_newline_flush_syntect(c: &mut Criterion) {
    let theme = Theme::default();
    let md = sample_markdown(200);
    let deltas = chunks(&md, 3);
    let hi = Arc::new(NoHighlight);

    c.bench_function(
        "markdown_view/streaming/mdstream/newline_flush/no_highlight",
        |b| {
            b.iter(|| {
                let mut view = MarkdownView::with_options(MarkdownViewOptions {
                    show_code_line_numbers: true,
                    ..Default::default()
                });
                view.set_highlighter(Some(hi.clone()));

                let mut stream = MarkdownStream::default();
                for d in &deltas {
                    let _ = stream.append(d);
                    if d.contains('\n') {
                        view.set_markdown(stream.display_markdown());
                        let _ = view.lines_for_width(96, &theme);
                    }
                }

                black_box(stream.display_markdown().len());
            })
        },
    );
}

fn bench_stream_view_newline_flush_syntect(c: &mut Criterion) {
    let theme = Theme::default();
    let md = sample_markdown(200);
    let deltas = chunks(&md, 3);
    let hi = Arc::new(NoHighlight);

    c.bench_function(
        "markdown_view/streaming/mdstream_view/newline_flush/no_highlight/trunc40",
        |b| {
            b.iter(|| {
                let mut view = MarkdownStreamView::with_options(
                    mdstream::Options::default(),
                    MarkdownViewOptions {
                        show_code_line_numbers: true,
                        ..Default::default()
                    },
                );
                view.set_highlighter(Some(hi.clone()));
                view.set_pending_code_fence_max_lines(Some(40));

                for d in &deltas {
                    let _ = view.append(d);
                    if d.contains('\n') {
                        let _ = view.total_lines_for_width(96, &theme);
                    }
                }

                black_box(view.total_lines_for_width(96, &theme));
            })
        },
    );
}

fn bench_stream_view_newline_flush_syntect_no_trunc(c: &mut Criterion) {
    let theme = Theme::default();
    let md = sample_markdown(200);
    let deltas = chunks(&md, 3);
    let hi = Arc::new(SyntectHighlighter::new());

    c.bench_function(
        "markdown_view/streaming/mdstream_view/newline_flush/syntect/no_trunc",
        |b| {
            b.iter(|| {
                let mut view = MarkdownStreamView::with_options(
                    mdstream::Options::default(),
                    MarkdownViewOptions {
                        show_code_line_numbers: true,
                        ..Default::default()
                    },
                );
                view.set_highlighter(Some(hi.clone()));
                view.set_pending_code_fence_max_lines(None);

                for d in &deltas {
                    let _ = view.append(d);
                    if d.contains('\n') {
                        let _ = view.total_lines_for_width(96, &theme);
                    }
                }

                black_box(view.total_lines_for_width(96, &theme));
            })
        },
    );
}

criterion_group!(
    benches,
    bench_streaming_mdstream_newline_flush_syntect,
    bench_stream_view_newline_flush_syntect,
    bench_stream_view_newline_flush_syntect_no_trunc
);
criterion_main!(benches);
