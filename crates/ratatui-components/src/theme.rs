use ratatui::style::Style;

#[derive(Clone, Debug)]
pub struct Theme {
    pub text_primary: Style,
    pub text_muted: Style,
    pub accent: Style,
    pub danger: Style,
    pub code_inline: Style,
    pub diff_add: Style,
    pub diff_del: Style,
}

impl Default for Theme {
    fn default() -> Self {
        use ratatui::style::Stylize;

        Self {
            text_primary: Style::default(),
            text_muted: Style::default().dark_gray(),
            accent: Style::default().cyan(),
            danger: Style::default().red(),
            code_inline: Style::default().cyan(),
            diff_add: Style::default().green(),
            diff_del: Style::default().red(),
        }
    }
}
