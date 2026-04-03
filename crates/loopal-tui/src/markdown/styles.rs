/// Markdown rendering styles for headings, inline elements, and structural markers.
use ratatui::prelude::*;

/// Collection of styles used by the markdown writer.
pub(super) struct MarkdownStyles {
    pub h1: Style,
    pub h2: Style,
    pub h3: Style,
    pub h4_h6: Style,
    pub emphasis: Style,
    pub strong: Style,
    pub strikethrough: Style,
    pub code_inline: Style,
    pub link: Style,
    pub link_url: Style,
    pub image_marker: Style,
    pub task_checked: Style,
    pub task_unchecked: Style,
    pub footnote_ref: Style,
    pub list_marker: Style,
    pub blockquote_marker: Style,
    pub rule: Style,
    pub table_border: Style,
    pub table_header: Style,
}

impl Default for MarkdownStyles {
    fn default() -> Self {
        Self {
            h1: Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            h2: Style::default().add_modifier(Modifier::BOLD),
            h3: Style::default().add_modifier(Modifier::BOLD | Modifier::ITALIC),
            h4_h6: Style::default().add_modifier(Modifier::ITALIC),
            emphasis: Style::default().add_modifier(Modifier::ITALIC),
            strong: Style::default().add_modifier(Modifier::BOLD),
            strikethrough: Style::default().add_modifier(Modifier::CROSSED_OUT),
            code_inline: Style::default().fg(Color::Cyan),
            link: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::UNDERLINED),
            link_url: Style::default().fg(Color::DarkGray),
            image_marker: Style::default().fg(Color::DarkGray),
            task_checked: Style::default().fg(Color::Green),
            task_unchecked: Style::default().fg(Color::DarkGray),
            footnote_ref: Style::default().fg(Color::DarkGray),
            list_marker: Style::default().fg(Color::LightBlue),
            blockquote_marker: Style::default().fg(Color::Green),
            rule: Style::default().fg(Color::DarkGray),
            table_border: Style::default().fg(Color::DarkGray),
            table_header: Style::default().add_modifier(Modifier::BOLD),
        }
    }
}

impl MarkdownStyles {
    /// Return the style for a heading level (1-based).
    pub fn heading(&self, level: u8) -> Style {
        match level {
            1 => self.h1,
            2 => self.h2,
            3 => self.h3,
            _ => self.h4_h6,
        }
    }
}
