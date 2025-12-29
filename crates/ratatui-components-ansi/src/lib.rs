use ansi_to_tui::IntoText;
use ratatui::text::Text;

pub fn ansi_text(input: &str) -> Text<'static> {
    input
        .into_text()
        .unwrap_or_else(|_| Text::from(input.to_string()))
}
