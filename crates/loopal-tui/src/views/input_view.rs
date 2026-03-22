/// Input view: single-line `> ` prompt with CJK cursor fix.
///
/// No border, no title — just a command input channel.
/// Shows inbox count when messages are queued: `> (2 queued) `.
/// Shows image count when images are attached: `> [img:2] `.
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthChar;

/// Render the input area as a single-line `> ` prompt.
pub fn render_input(
    f: &mut Frame,
    input: &str,
    cursor: usize,
    inbox_count: usize,
    image_count: usize,
    area: Rect,
) {
    if area.height == 0 {
        return;
    }

    let prefix = build_prefix(inbox_count, image_count);
    let prefix_width: usize = prefix.chars().map(|c| c.width().unwrap_or(0)).sum();

    let line = Line::from(vec![
        Span::styled(prefix, Style::default().fg(Color::DarkGray)),
        Span::raw(input.to_string()),
    ]);

    f.render_widget(Paragraph::new(line), area);

    // Cursor position: prefix display width + input display width up to cursor
    let input_width = display_width_up_to(input, cursor);
    f.set_cursor_position((
        area.x + (prefix_width + input_width) as u16,
        area.y,
    ));
}

/// Build the prompt prefix string from inbox and image counts.
fn build_prefix(inbox_count: usize, image_count: usize) -> String {
    match (inbox_count > 0, image_count > 0) {
        (true, true) => format!("> [img:{}] ({} queued) ", image_count, inbox_count),
        (true, false) => format!("> ({} queued) ", inbox_count),
        (false, true) => format!("> [img:{}] ", image_count),
        (false, false) => "> ".to_string(),
    }
}

/// Calculate the display width of a string up to byte position `pos`.
/// Uses UAX #11 via unicode-width for accurate CJK/emoji/fullwidth handling.
fn display_width_up_to(s: &str, byte_pos: usize) -> usize {
    let slice = &s[..byte_pos.min(s.len())];
    slice.chars().map(|c| c.width().unwrap_or(0)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_width() {
        assert_eq!(display_width_up_to("hello", 5), 5);
    }

    #[test]
    fn test_cjk_width() {
        let s = "你好世界";
        assert_eq!(display_width_up_to(s, 6), 4);
        assert_eq!(display_width_up_to(s, 12), 8);
    }

    #[test]
    fn test_mixed_width() {
        let s = "hi你好";
        assert_eq!(display_width_up_to(s, 2), 2);
        assert_eq!(display_width_up_to(s, 5), 4);
        assert_eq!(display_width_up_to(s, 8), 6);
    }

    #[test]
    fn test_empty() {
        assert_eq!(display_width_up_to("", 0), 0);
    }

    #[test]
    fn test_pos_beyond_length() {
        assert_eq!(display_width_up_to("abc", 100), 3);
    }

    #[test]
    fn test_prefix_variants() {
        assert_eq!(build_prefix(0, 0), "> ");
        assert_eq!(build_prefix(2, 0), "> (2 queued) ");
        assert_eq!(build_prefix(0, 1), "> [img:1] ");
        assert_eq!(build_prefix(3, 2), "> [img:2] (3 queued) ");
    }
}
