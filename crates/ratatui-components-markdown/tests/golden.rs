use ratatui::text::Line;
use ratatui_components::theme::Theme;
use ratatui_components_markdown::view::{
    LinkDestinationStyle, MarkdownView, MarkdownViewOptions, TableStyle,
};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
struct GoldenCase {
    name: &'static str,
    fixture: &'static str,
    width: u16,
    options: MarkdownViewOptions,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn read_fixture(rel: &str) -> String {
    fs::read_to_string(repo_root().join(rel)).expect("read fixture")
}

fn golden_path(case: &GoldenCase) -> PathBuf {
    repo_root()
        .join("docs/fixtures/golden/ratatui")
        .join(format!("{}__w{}.txt", case.name, case.width))
}

fn normalize(s: &str) -> String {
    let mut out = String::new();
    for (i, line) in s.replace("\r\n", "\n").split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(line.trim_end());
    }
    out.trim_end_matches('\n').to_string()
}

fn line_to_plain(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect::<Vec<_>>()
        .join("")
}

fn render(case: &GoldenCase) -> String {
    let md = read_fixture(case.fixture);
    let mut view = MarkdownView::with_options(case.options.clone());
    view.set_highlighter(None);
    view.set_markdown(&md);
    let theme = Theme::default();
    let lines = view
        .lines_for_width(case.width, &theme)
        .into_iter()
        .map(|l| line_to_plain(&l))
        .collect::<Vec<_>>()
        .join("\n");
    normalize(&lines)
}

fn update_goldens_enabled() -> bool {
    matches!(
        std::env::var("UPDATE_GOLDENS").as_deref(),
        Ok("1" | "true" | "yes")
    )
}

fn check_golden(case: GoldenCase) {
    let got = render(&case);
    let path = golden_path(&case);

    if update_goldens_enabled() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create golden dir");
        }
        fs::write(&path, format!("{got}\n")).expect("write golden");
        return;
    }

    let expected = fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing golden file: {}\nRun: UPDATE_GOLDENS=1 cargo test -p ratatui-components-markdown golden",
            path.display()
        )
    });
    let expected = normalize(&expected);
    assert_eq!(
        got,
        expected,
        "golden mismatch: {}\nRun: UPDATE_GOLDENS=1 cargo test -p ratatui-components-markdown golden",
        path.display()
    );
}

fn glow_like_options() -> MarkdownViewOptions {
    MarkdownViewOptions {
        preserve_new_lines: true,
        show_link_destinations: true,
        show_heading_markers: true,
        glow_compat_relative_paths: true,
        padding_left: 2,
        padding_right: 2,
        blockquote_prefix: "  ".to_string(),
        code_block_indent: 2,
        code_block_indent_in_blockquote: 2,
        footnote_hanging_indent: false,
        glow_compat_loose_list_join: true,
        glow_compat_post_list_blank_lines: 3,
        table_style: TableStyle::Glow,
        glow_compat_quote_list_wrap: true,
        footnotes_at_end: false,
        base_url: None,
        link_destination_style: LinkDestinationStyle::Space,
        ..MarkdownViewOptions::default()
    }
}

#[test]
fn golden_glow_parity_w80() {
    check_golden(GoldenCase {
        name: "glow_parity__glow_like",
        fixture: "docs/fixtures/glow_parity.md",
        width: 80,
        options: glow_like_options(),
    });
}

#[test]
fn golden_glow_parity_w40() {
    check_golden(GoldenCase {
        name: "glow_parity__glow_like",
        fixture: "docs/fixtures/glow_parity.md",
        width: 40,
        options: glow_like_options(),
    });
}

#[test]
#[ignore = "Parity driver: compare our plain output to glow's plain output snapshots"]
fn parity_report_against_glow_snapshots() {
    let case = GoldenCase {
        name: "glow_parity__glow_like",
        fixture: "docs/fixtures/glow_parity.md",
        width: 80,
        options: glow_like_options(),
    };
    let got = render(&case);

    let glow_path = repo_root()
        .join("docs/fixtures/golden/glow")
        .join(format!("{}__w{}.txt", case.name, case.width));
    let glow = fs::read_to_string(&glow_path).expect("read glow snapshot");
    let glow = normalize(&glow);

    assert_eq!(
        got,
        glow,
        "parity mismatch vs glow snapshot: {}",
        glow_path.display()
    );
}
