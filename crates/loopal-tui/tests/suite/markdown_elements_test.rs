use loopal_tui::markdown::render_markdown;
use ratatui::prelude::*;

fn lines_text(lines: &[Line<'_>]) -> Vec<String> {
    lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
        .collect()
}

// --- Task lists ---

#[test]
fn test_task_list_unchecked() {
    let input = "- [ ] todo item";
    let lines = render_markdown(input, 80);
    let texts = lines_text(&lines);
    assert!(
        texts
            .iter()
            .any(|t| t.contains("[ ]") && t.contains("todo"))
    );
}

#[test]
fn test_task_list_checked() {
    let input = "- [x] done item";
    let lines = render_markdown(input, 80);
    let texts = lines_text(&lines);
    assert!(
        texts
            .iter()
            .any(|t| t.contains("[x]") && t.contains("done"))
    );
}

#[test]
fn test_task_list_checked_has_green_style() {
    let input = "- [x] completed";
    let lines = render_markdown(input, 80);
    let span = lines
        .iter()
        .flat_map(|l| &l.spans)
        .find(|s| s.content.contains("[x]"));
    assert!(span.is_some());
    assert_eq!(span.unwrap().style.fg, Some(Color::Green));
}

// --- Link with URL ---

#[test]
fn test_link_shows_url() {
    let input = "[click](https://example.com)";
    let lines = render_markdown(input, 80);
    let texts = lines_text(&lines);
    let full = texts.join("");
    assert!(full.contains("click"), "link text");
    assert!(full.contains("https://example.com"), "URL shown");
}

// --- Image ---

#[test]
fn test_image_shows_alt_text() {
    let input = "![alt text](image.png)";
    let lines = render_markdown(input, 80);
    let texts = lines_text(&lines);
    let full = texts.join("");
    assert!(full.contains("image:"), "image marker");
    assert!(full.contains("alt text"), "alt text shown");
}

#[test]
fn test_image_no_alt_shows_placeholder() {
    let input = "![](image.png)";
    let lines = render_markdown(input, 80);
    let texts = lines_text(&lines);
    let full = texts.join("");
    assert!(full.contains("[image]"), "placeholder for empty alt");
}

// --- Footnote reference ---

#[test]
fn test_footnote_reference() {
    let input = "text[^1] more";
    let lines = render_markdown(input, 80);
    let texts = lines_text(&lines);
    let full = texts.join("");
    assert!(full.contains("[^1]"), "footnote ref shown");
}
