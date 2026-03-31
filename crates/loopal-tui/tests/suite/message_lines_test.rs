use loopal_session::types::SessionMessage;
use loopal_tui::views::progress::{message_to_lines, streaming_to_lines};

fn msg(role: &str, content: &str) -> SessionMessage {
    SessionMessage {
        role: role.to_string(),
        content: content.to_string(),
        tool_calls: Vec::new(),
        image_count: 0,
        skill_info: None,
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
    // No label in instruction model — content lines + 1 empty separator
    // 150 chars at width 40 → at least 4 visual content lines
    let content_lines = lines.len() - 1; // subtract trailing empty
    assert!(
        content_lines >= 4,
        "expected >= 4 content lines at width 40, got {content_lines}"
    );
}

#[test]
fn test_short_line_no_extra_wrap() {
    let m = msg("user", "hello world");
    let lines = message_to_lines(&m, 80);
    // "▎ hello world" + empty separator = 2
    assert_eq!(lines.len(), 2);
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
    // "▎ " prefix + CJK in 10-col width → at least 2 content lines
    let content_lines = lines_10.len() - 1; // subtract trailing empty
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
    // No label in instruction model — just wrapped content lines
    assert!(lines.len() > 1, "streaming should wrap long text");
}

#[test]
fn test_streaming_empty_returns_nothing() {
    let lines = streaming_to_lines("", 80);
    assert!(lines.is_empty());
}

// --- Streaming markdown rendering ---

#[test]
fn test_streaming_renders_bold_markdown() {
    // streaming_to_lines should render markdown, not show raw **markers**.
    let lines = streaming_to_lines("Hello **bold** world", 80);
    let texts = lines_text(&lines);
    let full: String = texts.join("");
    assert!(full.contains("bold"), "bold text present");
    assert!(!full.contains("**"), "raw ** markers should not appear");
}

#[test]
fn test_streaming_renders_code_block() {
    let input = "text\n\n```rust\nfn main() {}\n```\n\nmore";
    let lines = streaming_to_lines(input, 80);
    let texts = lines_text(&lines);
    let full = texts.join(" ");
    assert!(full.contains("fn main()"), "code content present");
    // Raw ``` fence markers should not appear as content.
    assert!(!full.contains("```"), "raw fence markers should not appear");
}

#[test]
fn test_streaming_renders_heading() {
    let lines = streaming_to_lines("## Title\n\nBody", 80);
    // Find the span containing "Title" and verify it has bold style.
    let title_span = lines
        .iter()
        .flat_map(|l| &l.spans)
        .find(|s| s.content.contains("Title"));
    assert!(title_span.is_some(), "Title text should be present");
    assert!(
        title_span
            .unwrap()
            .style
            .add_modifier
            .contains(ratatui::prelude::Modifier::BOLD),
        "heading should be bold"
    );
}

#[test]
fn test_empty_content_produces_only_separator() {
    // Assistant with empty content — no label, just trailing separator
    let m = msg("assistant", "");
    let lines = message_to_lines(&m, 80);
    let texts = lines_text(&lines);
    assert_eq!(texts.last().unwrap(), "");
    // Only empty separator line (no label in instruction model)
    assert_eq!(lines.len(), 1);
}

#[test]
fn test_multiline_content_preserves_line_breaks() {
    let m = msg("user", "line1\nline2\nline3");
    let lines = message_to_lines(&m, 80);
    let texts = lines_text(&lines);
    // "▎ line1" + "▎ line2" + "▎ line3" + separator = 4
    assert_eq!(lines.len(), 4);
    assert!(texts[0].contains("line1"));
    assert!(texts[1].contains("line2"));
    assert!(texts[2].contains("line3"));
}

#[test]
fn test_user_message_has_prompt_prefix() {
    let m = msg("user", "hello");
    let lines = message_to_lines(&m, 80);
    let texts = lines_text(&lines);
    assert!(
        texts[0].starts_with("▎ "),
        "user msg should start with '▎ '"
    );
}

#[test]
fn test_long_url_falls_back_to_char_break() {
    // Long string with no spaces should still wrap (break_words default)
    let url = "a".repeat(200);
    let m = msg("user", &url);
    let lines = message_to_lines(&m, 40);
    let content_lines = lines.len() - 1; // subtract trailing empty
    assert!(
        content_lines >= 5,
        "200-char no-space string at width 40 should produce >= 5 lines"
    );
}
