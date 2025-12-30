use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffLineKind {
    FileHeader,
    HunkHeader,
    Add,
    Del,
    Context,
    Meta,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub raw: String,
    pub content: String,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    pub language_hint: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ParsedDiff {
    pub lines: Vec<DiffLine>,
    pub max_old_lineno: u32,
    pub max_new_lineno: u32,
    pub max_content_width: u16,
}

pub fn parse_unified_diff(input: &str) -> ParsedDiff {
    let mut out = ParsedDiff::default();

    let mut in_hunk = false;
    let mut old_lineno: u32 = 0;
    let mut new_lineno: u32 = 0;
    let mut language_hint: Option<String> = None;

    for raw in input.lines() {
        let raw = if raw.contains('\t') {
            raw.replace('\t', "    ")
        } else {
            raw.to_string()
        };

        let (kind, old_no, new_no, advance_old, advance_new) =
            if let Some((o, n)) = parse_hunk_header(&raw) {
                in_hunk = true;
                old_lineno = o;
                new_lineno = n;
                (DiffLineKind::HunkHeader, None, None, false, false)
            } else if raw.starts_with("diff --git ")
                || raw.starts_with("index ")
                || raw.starts_with("new file mode ")
                || raw.starts_with("deleted file mode ")
                || raw.starts_with("similarity index ")
                || raw.starts_with("rename from ")
                || raw.starts_with("rename to ")
            {
                in_hunk = false;
                if let Some(ext) = parse_diff_git_extension(&raw) {
                    language_hint = Some(ext);
                }
                (DiffLineKind::FileHeader, None, None, false, false)
            } else if raw.starts_with("--- ") || raw.starts_with("+++ ") {
                in_hunk = false;
                if let Some(ext) = parse_file_header_extension(&raw) {
                    language_hint = Some(ext);
                }
                (DiffLineKind::FileHeader, None, None, false, false)
            } else if in_hunk {
                match raw.as_bytes().first().copied() {
                    Some(b' ') => (
                        DiffLineKind::Context,
                        Some(old_lineno),
                        Some(new_lineno),
                        true,
                        true,
                    ),
                    Some(b'+') => (DiffLineKind::Add, None, Some(new_lineno), false, true),
                    Some(b'-') => (DiffLineKind::Del, Some(old_lineno), None, true, false),
                    Some(b'\\') => (DiffLineKind::Meta, None, None, false, false),
                    _ => (DiffLineKind::Meta, None, None, false, false),
                }
            } else {
                (DiffLineKind::Meta, None, None, false, false)
            };

        let content = displayed_content(kind, &raw);

        if let Some(v) = old_no {
            out.max_old_lineno = out.max_old_lineno.max(v);
        }
        if let Some(v) = new_no {
            out.max_new_lineno = out.max_new_lineno.max(v);
        }
        out.max_content_width = out
            .max_content_width
            .max(UnicodeWidthStr::width(content.as_str()) as u16);

        out.lines.push(DiffLine {
            kind,
            raw,
            content,
            old_lineno: old_no,
            new_lineno: new_no,
            language_hint: language_hint.clone(),
        });

        if advance_old {
            old_lineno = old_lineno.saturating_add(1);
        }
        if advance_new {
            new_lineno = new_lineno.saturating_add(1);
        }
    }

    out
}

fn displayed_content(kind: DiffLineKind, raw: &str) -> String {
    match kind {
        DiffLineKind::Add | DiffLineKind::Del | DiffLineKind::Context => {
            raw.strip_prefix(['+', '-', ' ']).unwrap_or(raw).to_string()
        }
        _ => raw.to_string(),
    }
}

fn parse_hunk_header(line: &str) -> Option<(u32, u32)> {
    if !line.starts_with("@@") {
        return None;
    }
    let after = line.strip_prefix("@@")?.trim_start();
    let after = after.strip_prefix('-')?;
    let (old_part, rest) = after.split_once(' ')?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('+')?;
    let (new_part, _rest2) = rest.split_once(' ')?;

    let old_start = old_part.split_once(',').map_or(old_part, |(a, _)| a);
    let new_start = new_part.split_once(',').map_or(new_part, |(a, _)| a);

    let old_start: u32 = old_start.parse().ok()?;
    let new_start: u32 = new_start.parse().ok()?;
    Some((old_start, new_start))
}

fn parse_file_header_extension(line: &str) -> Option<String> {
    let path = line
        .strip_prefix("+++ ")
        .or_else(|| line.strip_prefix("--- "))?
        .trim();
    file_extension(path).map(|s| s.to_string())
}

fn parse_diff_git_extension(line: &str) -> Option<String> {
    let rest = line.strip_prefix("diff --git ")?;
    let (_a, b) = rest.split_once(' ')?;
    file_extension(b).map(|s| s.to_string())
}

fn file_extension(path: &str) -> Option<&str> {
    let path = path.trim();
    let path = path
        .strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path);
    let path = path.rsplit_once('/').map_or(path, |(_, name)| name);
    let (_, ext) = path.rsplit_once('.')?;
    if ext.is_empty() { None } else { Some(ext) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hunk_and_line_numbers() {
        let diff = "\
diff --git a/a.txt b/a.txt
--- a/a.txt
+++ b/a.txt
@@ -3,2 +10,3 @@
 line
-old
+new
";
        let parsed = parse_unified_diff(diff);
        assert!(
            parsed
                .lines
                .iter()
                .any(|l| l.kind == DiffLineKind::HunkHeader)
        );

        let ctx = parsed
            .lines
            .iter()
            .find(|l| l.kind == DiffLineKind::Context)
            .unwrap();
        assert_eq!(ctx.old_lineno, Some(3));
        assert_eq!(ctx.new_lineno, Some(10));
        assert_eq!(ctx.content, "line");

        let del = parsed
            .lines
            .iter()
            .find(|l| l.kind == DiffLineKind::Del)
            .unwrap();
        assert_eq!(del.old_lineno, Some(4));
        assert_eq!(del.new_lineno, None);
        assert_eq!(del.content, "old");

        let add = parsed
            .lines
            .iter()
            .find(|l| l.kind == DiffLineKind::Add)
            .unwrap();
        assert_eq!(add.old_lineno, None);
        assert_eq!(add.new_lineno, Some(11));
        assert_eq!(add.content, "new");
    }

    #[test]
    fn parses_hunk_header_start_positions() {
        assert_eq!(parse_hunk_header("@@ -1 +2 @@"), Some((1, 2)));
        assert_eq!(parse_hunk_header("@@ -3,10 +7,9 @@"), Some((3, 7)));
        assert_eq!(parse_hunk_header("not a hunk"), None);
    }

    #[test]
    fn captures_language_hint_from_headers() {
        let diff = "\
diff --git a/main.rs b/main.rs
--- a/main.rs
+++ b/main.rs
@@ -1 +1 @@
 a
";
        let parsed = parse_unified_diff(diff);
        let ctx = parsed
            .lines
            .iter()
            .find(|l| l.kind == DiffLineKind::Context)
            .unwrap();
        assert_eq!(ctx.language_hint.as_deref(), Some("rs"));
    }
}
