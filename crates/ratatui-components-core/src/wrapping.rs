use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WrapMode {
    #[default]
    None,
    Word,
}

#[derive(Clone, Debug, Default)]
pub struct WrapCache {
    raw: Vec<String>,
    wrapped: Vec<String>,
    wrapped_src_idx: Vec<usize>,
    wrap_cols: Option<u16>,
    mode: WrapMode,
    content_w: u16,
    content_h: u16,
}

impl WrapCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_content(&mut self, lines: Vec<String>) {
        self.raw = lines;
        self.invalidate();
    }

    pub fn set_mode(&mut self, mode: WrapMode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        self.invalidate();
    }

    pub fn set_width(&mut self, width: u16) {
        if self.wrap_cols == Some(width) {
            return;
        }
        self.wrap_cols = Some(width);
        self.rebuild();
    }

    pub fn raw_lines(&self) -> &[String] {
        &self.raw
    }

    pub fn wrapped_lines(&self) -> &[String] {
        &self.wrapped
    }

    pub fn wrapped_src_indices(&self) -> &[usize] {
        &self.wrapped_src_idx
    }

    pub fn content_width(&self) -> u16 {
        self.content_w
    }

    pub fn content_height(&self) -> u16 {
        self.content_h
    }

    fn invalidate(&mut self) {
        self.wrapped.clear();
        self.wrapped_src_idx.clear();
        self.content_w = 0;
        self.content_h = 0;
        self.wrap_cols = None;
    }

    fn rebuild(&mut self) {
        let Some(width) = self.wrap_cols else {
            return;
        };
        match self.mode {
            WrapMode::None => self.no_wrap(),
            WrapMode::Word => self.word_wrap(width),
        }
        self.content_h = self.wrapped.len() as u16;
        self.content_w = self
            .wrapped
            .iter()
            .map(|l| UnicodeWidthStr::width(l.as_str()) as u16)
            .max()
            .unwrap_or(0);
    }

    fn no_wrap(&mut self) {
        self.wrapped = self
            .raw
            .iter()
            .map(|l| normalize_tabs(l))
            .collect::<Vec<_>>();
        self.wrapped_src_idx = (0..self.raw.len()).collect();
    }

    fn word_wrap(&mut self, width: u16) {
        if width == 0 {
            self.no_wrap();
            return;
        }

        let max_cols = width as usize;
        let mut out: Vec<String> = Vec::new();
        let mut out_idx: Vec<usize> = Vec::new();

        for (raw_idx, raw) in self.raw.iter().enumerate() {
            let raw = normalize_tabs(raw);
            if raw.is_empty() {
                out.push(String::new());
                out_idx.push(raw_idx);
                continue;
            }

            let mut line = String::new();
            let mut line_cols = 0usize;
            let mut last_soft_idx: Option<usize> = None;

            for ch in raw.chars() {
                if ch == '\n' {
                    out.push(std::mem::take(&mut line));
                    out_idx.push(raw_idx);
                    line_cols = 0;
                    last_soft_idx = None;
                    continue;
                }

                let w = UnicodeWidthChar::width(ch).unwrap_or(0);
                if line_cols.saturating_add(w) > max_cols {
                    if let Some(split) = last_soft_idx {
                        let (prefix, rest) = line.split_at(split);
                        out.push(prefix.trim_end().to_string());
                        out_idx.push(raw_idx);
                        line = rest.trim_start().to_string();
                        last_soft_idx = None;
                    } else if !line.is_empty() {
                        out.push(std::mem::take(&mut line));
                        out_idx.push(raw_idx);
                    }
                }

                if line.is_empty() && ch.is_whitespace() {
                    continue;
                }

                if ch.is_whitespace()
                    || matches!(
                        ch,
                        ',' | ';' | '.' | ':' | ')' | ']' | '}' | '|' | '/' | '?' | '!' | '-' | '_'
                    )
                {
                    last_soft_idx = Some(line.len());
                }

                line.push(ch);
                line_cols = UnicodeWidthStr::width(line.as_str());
            }

            if !line.is_empty() {
                out.push(line);
                out_idx.push(raw_idx);
            }
        }

        self.wrapped = out;
        self.wrapped_src_idx = out_idx;
    }
}

fn normalize_tabs(s: &str) -> String {
    if s.contains('\t') {
        s.replace('\t', "    ")
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_wrap_splits_and_maps_source_indices() {
        let mut cache = WrapCache::new();
        cache.set_content(vec!["hello world".to_string()]);
        cache.set_mode(WrapMode::Word);
        cache.set_width(5);

        assert_eq!(
            cache.wrapped_lines(),
            &["hello".to_string(), "world".to_string()]
        );
        assert_eq!(cache.wrapped_src_indices(), &[0, 0]);
        assert_eq!(cache.content_height(), 2);
        assert_eq!(cache.content_width(), 5);
    }

    #[test]
    fn no_wrap_expands_tabs() {
        let mut cache = WrapCache::new();
        cache.set_content(vec!["a\tb".to_string()]);
        cache.set_mode(WrapMode::None);
        cache.set_width(80);
        assert_eq!(cache.wrapped_lines(), &["a    b".to_string()]);
    }

    #[test]
    fn set_content_invalidates_until_width_is_set() {
        let mut cache = WrapCache::new();
        cache.set_content(vec!["x".to_string()]);
        cache.set_mode(WrapMode::None);
        assert_eq!(cache.content_height(), 0);
        cache.set_width(10);
        assert_eq!(cache.content_height(), 1);
    }
}
