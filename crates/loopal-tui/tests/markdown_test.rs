use loopal_tui::markdown::render_markdown;
use ratatui::prelude::*;

fn lines_text(lines: &[Line<'_>]) -> Vec<String> {
    lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
        .collect()
}

fn has_modifier(line: &Line<'_>, modifier: Modifier) -> bool {
    line.spans
        .iter()
        .any(|s| s.style.add_modifier.contains(modifier))
}

// --- Paragraph wrapping ---

#[test]
fn test_paragraph_wraps_to_width() {
    let input = "word ".repeat(20);
    let lines = render_markdown(&input, 30);
    // Should produce multiple wrapped lines + trailing blank
    let non_empty: Vec<_> = lines.iter().filter(|l| !lines_text(&[(*l).clone()])[0].is_empty()).collect();
    assert!(non_empty.len() > 1, "long paragraph should wrap");
}

#[test]
fn test_short_paragraph_single_line() {
    let lines = render_markdown("hello world", 80);
    let texts = lines_text(&lines);
    assert!(texts.iter().any(|t| t.contains("hello world")));
}

// --- Heading styles ---

#[test]
fn test_h1_bold_underlined() {
    let lines = render_markdown("# Title", 80);
    let heading = lines.iter().find(|l| {
        l.spans.iter().any(|s| s.content.contains("Title"))
    });
    assert!(heading.is_some(), "heading line should exist");
    let h = heading.unwrap();
    assert!(has_modifier(h, Modifier::BOLD));
    assert!(has_modifier(h, Modifier::UNDERLINED));
}

#[test]
fn test_h2_bold() {
    let lines = render_markdown("## Subtitle", 80);
    let heading = lines.iter().find(|l| {
        l.spans.iter().any(|s| s.content.contains("Subtitle"))
    });
    assert!(heading.is_some());
    assert!(has_modifier(heading.unwrap(), Modifier::BOLD));
}

// --- Inline styles ---

#[test]
fn test_bold_text() {
    let lines = render_markdown("some **bold** text", 80);
    let bold_span = lines.iter().flat_map(|l| &l.spans).find(|s| {
        s.content.contains("bold")
    });
    assert!(bold_span.is_some());
    assert!(bold_span.unwrap().style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn test_italic_text() {
    let lines = render_markdown("some *italic* text", 80);
    let span = lines.iter().flat_map(|l| &l.spans).find(|s| {
        s.content.contains("italic")
    });
    assert!(span.is_some());
    assert!(span.unwrap().style.add_modifier.contains(Modifier::ITALIC));
}

#[test]
fn test_inline_code() {
    let lines = render_markdown("use `foo()` here", 80);
    let span = lines.iter().flat_map(|l| &l.spans).find(|s| {
        s.content.contains("foo()")
    });
    assert!(span.is_some());
    assert_eq!(span.unwrap().style.fg, Some(Color::Cyan));
}

// --- Lists ---

#[test]
fn test_unordered_list() {
    let input = "- item one\n- item two\n- item three";
    let lines = render_markdown(input, 80);
    let texts = lines_text(&lines);
    assert!(texts.iter().any(|t| t.contains("item one")));
    assert!(texts.iter().any(|t| t.contains("item two")));
}

#[test]
fn test_ordered_list() {
    let input = "1. first\n2. second\n3. third";
    let lines = render_markdown(input, 80);
    let texts = lines_text(&lines);
    assert!(texts.iter().any(|t| t.contains("1.")));
    assert!(texts.iter().any(|t| t.contains("first")));
}

// --- Blockquote ---

#[test]
fn test_blockquote_prefix() {
    let lines = render_markdown("> quoted text", 80);
    let texts = lines_text(&lines);
    assert!(texts.iter().any(|t| t.contains(">") && t.contains("quoted")));
}

// --- Horizontal rule ---

#[test]
fn test_horizontal_rule() {
    let lines = render_markdown("above\n\n---\n\nbelow", 80);
    let texts = lines_text(&lines);
    assert!(texts.iter().any(|t| t.contains("─")));
}

// --- Mixed content ---

#[test]
fn test_mixed_paragraph_and_list() {
    let input = "Hello world.\n\n- item one\n- item two\n\nGoodbye.";
    let lines = render_markdown(input, 80);
    let texts = lines_text(&lines);
    assert!(texts.iter().any(|t| t.contains("Hello")));
    assert!(texts.iter().any(|t| t.contains("item one")));
    assert!(texts.iter().any(|t| t.contains("Goodbye")));
}
