use crate::keymap::Binding;
use crate::render;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;

#[derive(Clone, Debug)]
pub struct HelpBarOptions {
    pub style: Style,
    pub key_style: Style,
    pub separator: String,
    pub space: String,
}

impl Default for HelpBarOptions {
    fn default() -> Self {
        Self {
            style: Style::default(),
            key_style: Style::default(),
            separator: " • ".to_string(),
            space: " ".to_string(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct HelpBar {
    bindings: Vec<Binding>,
    options: HelpBarOptions,
}

impl HelpBar {
    pub fn new(bindings: Vec<Binding>) -> Self {
        Self {
            bindings,
            options: HelpBarOptions::default(),
        }
    }

    pub fn with_options(bindings: Vec<Binding>, options: HelpBarOptions) -> Self {
        Self { bindings, options }
    }

    pub fn set_bindings(&mut self, bindings: Vec<Binding>) {
        self.bindings = bindings;
    }

    pub fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let spans = self.to_spans();
        buf.set_style(area, self.options.style);
        render::render_spans_clipped(
            area.x,
            area.y,
            0,
            area.width,
            buf,
            &spans,
            self.options.style,
        );
    }

    fn to_spans(&self) -> Vec<Span<'static>> {
        let mut spans: Vec<Span<'static>> = Vec::new();
        for (i, b) in self.bindings.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(
                    self.options.separator.clone(),
                    self.options.style,
                ));
            }
            spans.push(Span::styled(b.help_key.clone(), self.options.key_style));
            spans.push(Span::styled(self.options.space.clone(), self.options.style));
            spans.push(Span::styled(b.help_desc.clone(), self.options.style));
        }
        spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap;
    use ratatui::buffer::Buffer;

    #[test]
    fn help_bar_renders_narrow_width() {
        let bindings = vec![keymap::Binding::new(
            "q",
            "quit",
            vec![keymap::key_char('q')],
        )];
        let hb = HelpBar::new(bindings);
        let mut buf = Buffer::empty(Rect::new(0, 0, 3, 1));
        hb.render_ref(Rect::new(0, 0, 3, 1), &mut buf);
    }
}
