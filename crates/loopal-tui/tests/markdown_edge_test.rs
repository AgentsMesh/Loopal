use loopal_tui::markdown::render_markdown;
use ratatui::prelude::*;

fn lines_text(lines: &[Line<'_>]) -> Vec<String> {
    lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
        .collect()
}

// --- Empty input ---

#[test]
fn test_empty_input_returns_empty() {
    let lines = render_markdown("", 80);
    assert!(lines.is_empty());
}

// --- Nested styles ---

#[test]
fn test_bold_italic_nested() {
    let lines = render_markdown("***bold italic***", 80);
    let span = lines.iter().flat_map(|l| &l.spans).find(|s| {
        s.content.contains("bold italic")
    });
    assert!(span.is_some());
    let s = span.unwrap();
    assert!(s.style.add_modifier.contains(Modifier::BOLD));
    assert!(s.style.add_modifier.contains(Modifier::ITALIC));
}

#[test]
fn test_strikethrough() {
    let lines = render_markdown("~~deleted~~", 80);
    let span = lines.iter().flat_map(|l| &l.spans).find(|s| {
        s.content.contains("deleted")
    });
    assert!(span.is_some());
    assert!(
        span.unwrap()
            .style
            .add_modifier
            .contains(Modifier::CROSSED_OUT)
    );
}

// --- Width edge cases ---

#[test]
fn test_width_1_no_panic() {
    let lines = render_markdown("hello world", 1);
    assert!(!lines.is_empty());
}

#[test]
fn test_very_narrow_code_block() {
    let input = "```\nshort\n```";
    let lines = render_markdown(input, 5);
    let texts = lines_text(&lines);
    assert!(texts.iter().any(|t| t.contains("short")));
}

// --- Ultra-long single line ---

#[test]
fn test_ultra_long_paragraph() {
    let long = "word ".repeat(1000);
    let lines = render_markdown(&long, 40);
    assert!(lines.len() > 10, "should produce many wrapped lines");
}

// --- Multiple blank lines collapse ---

#[test]
fn test_consecutive_paragraphs() {
    let input = "para one\n\npara two\n\npara three";
    let lines = render_markdown(input, 80);
    let texts = lines_text(&lines);
    assert!(texts.iter().any(|t| t.contains("para one")));
    assert!(texts.iter().any(|t| t.contains("para two")));
    assert!(texts.iter().any(|t| t.contains("para three")));
}

// --- Link rendering ---

#[test]
fn test_link_styled() {
    let lines = render_markdown("[click](https://example.com)", 80);
    let span = lines.iter().flat_map(|l| &l.spans).find(|s| {
        s.content.contains("click")
    });
    assert!(span.is_some());
    assert_eq!(span.unwrap().style.fg, Some(Color::Cyan));
    assert!(
        span.unwrap()
            .style
            .add_modifier
            .contains(Modifier::UNDERLINED)
    );
}

// --- Nested blockquote ---

#[test]
fn test_nested_blockquote() {
    let input = "> outer\n> > inner";
    let lines = render_markdown(input, 80);
    let texts = lines_text(&lines);
    assert!(texts.iter().any(|t| t.contains("outer")));
    assert!(texts.iter().any(|t| t.contains("inner")));
}

// --- Nested list ---

#[test]
fn test_nested_list() {
    let input = "- a\n  - b\n  - c\n- d";
    let lines = render_markdown(input, 80);
    let texts = lines_text(&lines);
    assert!(texts.len() >= 4, "nested list should produce at least 4 lines");
}
