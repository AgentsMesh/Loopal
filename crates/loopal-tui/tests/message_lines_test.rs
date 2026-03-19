use loopal_session::types::DisplayMessage;
use loopal_tui::views::progress::{message_to_lines, streaming_to_lines};

fn msg(role: &str, content: &str) -> DisplayMessage {
    DisplayMessage {
        role: role.to_string(),
        content: content.to_string(),
        tool_calls: Vec::new(),
    }
}

fn lines_text(lines: &[ratatui::prelude::Line<'_>]) -> Vec<String> {
    lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect()
}

// --- Pre-wrap behavior ---

#[test]
fn test_long_line_wraps_into_multiple_visual_lines() {
    let long = "word ".repeat(30); // 150 chars
    let m = msg("assistant", &long);
    let lines = message_to_lines(&m, 40);
    // 1 role label + multiple content lines + 1 empty separator
    // 150 chars at width 40 → at least 4 visual content lines
    let content_lines = lines.len() - 2; // subtract label + trailing empty
    assert!(
        content_lines >= 4,
        "expected >= 4 content lines at width 40, got {content_lines}"
    );
}

#[test]
fn test_short_line_no_extra_wrap() {
    let m = msg("user", "hello world");
    let lines = message_to_lines(&m, 80);
    // label + 1 content line + empty separator = 3
    assert_eq!(lines.len(), 3);
}

#[test]
fn test_visual_lines_equal_len() {
    // Core invariant: lines.len() == visual line count
    let long = "a]b ".repeat(50);
    let m = msg("assistant", &long);
    let lines_wide = message_to_lines(&m, 200);
    let lines_narrow = message_to_lines(&m, 20);
    assert!(
        lines_narrow.len() > lines_wide.len(),
        "narrower width must produce more visual lines"
    );
}

// --- CJK double-width ---

#[test]
fn test_cjk_double_width_wraps_correctly() {
    // Each CJK char is 2 columns wide. 10 chars = 20 columns.
    let cjk = "你好世界测试中文双宽";
    let m = msg("user", cjk);
    let lines_10 = message_to_lines(&m, 10);
    // 20 display-columns in 10-col width → at least 2 content lines
    let content_lines = lines_10.len() - 2;
    assert!(
        content_lines >= 2,
        "expected >= 2 CJK content lines at width 10, got {content_lines}"
    );
}

// --- Streaming wrap ---

#[test]
fn test_streaming_wraps_long_text() {
    let long = "stream ".repeat(20);
    let lines = streaming_to_lines(&long, 30);
    // 1 label + multiple wrapped content lines
    assert!(lines.len() > 2, "streaming should wrap long text");
}

#[test]
fn test_streaming_empty_returns_nothing() {
    let lines = streaming_to_lines("", 80);
    assert!(lines.is_empty());
}

// --- Edge cases ---

#[test]
fn test_empty_content_produces_label_and_separator() {
    let m = msg("assistant", "");
    let lines = message_to_lines(&m, 80);
    let texts = lines_text(&lines);
    assert_eq!(texts.first().unwrap(), "Agent: ");
    assert_eq!(texts.last().unwrap(), "");
    // label + empty separator = 2
    assert_eq!(lines.len(), 2);
}

#[test]
fn test_multiline_content_preserves_line_breaks() {
    let m = msg("user", "line1\nline2\nline3");
    let lines = message_to_lines(&m, 80);
    let texts = lines_text(&lines);
    // label + 3 content lines + separator = 5
    assert_eq!(lines.len(), 5);
    assert_eq!(texts[1], "line1");
    assert_eq!(texts[2], "line2");
    assert_eq!(texts[3], "line3");
}

#[test]
fn test_long_url_falls_back_to_char_break() {
    // Long string with no spaces should still wrap (break_words default)
    let url = "a".repeat(200);
    let m = msg("user", &url);
    let lines = message_to_lines(&m, 40);
    let content_lines = lines.len() - 2;
    assert!(
        content_lines >= 5,
        "200-char no-space string at width 40 should produce >= 5 lines"
    );
}
