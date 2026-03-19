use loopal_tui::markdown::render_markdown;
use ratatui::prelude::*;

fn lines_text(lines: &[Line<'_>]) -> Vec<String> {
    lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
        .collect()
}

// --- styled_wrap via render_markdown ---

#[test]
fn test_bold_text_wraps_preserving_style() {
    // A long bold paragraph should wrap while keeping bold style
    let input = format!("**{}**", "bold ".repeat(30).trim());
    let lines = render_markdown(&input, 40);
    let bold_lines: Vec<_> = lines
        .iter()
        .filter(|l| {
            l.spans
                .iter()
                .any(|s| s.style.add_modifier.contains(Modifier::BOLD))
        })
        .collect();
    assert!(bold_lines.len() > 1, "bold text should wrap into multiple lines");
}

#[test]
fn test_mixed_style_paragraph_wraps() {
    let input = "Normal **bold** and *italic* mixed together in a sentence.";
    let lines = render_markdown(input, 20);
    assert!(lines.len() > 1, "mixed paragraph should wrap at width 20");
}

#[test]
fn test_inline_code_in_paragraph_wraps() {
    let input = format!("Use `command` to {} end.", "do something ".repeat(10));
    let lines = render_markdown(&input, 40);
    assert!(lines.len() > 1);
    // Inline code span should be present
    let has_code = lines.iter().flat_map(|l| &l.spans).any(|s| {
        s.content.contains("command") && s.style.fg == Some(Color::Cyan)
    });
    assert!(has_code, "inline code style should be preserved after wrap");
}

// --- Edge: single span no wrap ---

#[test]
fn test_single_short_span_no_wrap() {
    let lines = render_markdown("hi", 80);
    let texts = lines_text(&lines);
    assert!(texts.iter().any(|t| t.contains("hi")));
}

// --- Edge: empty spans ---

#[test]
fn test_whitespace_only_input() {
    let lines = render_markdown("   \n   \n", 80);
    // Should not panic
    let _ = lines;
}

// --- Heading wraps at narrow width ---

#[test]
fn test_heading_wraps_narrow() {
    let input = "# This is a very long heading that should wrap";
    let lines = render_markdown(input, 20);
    let heading_lines: Vec<_> = lines
        .iter()
        .filter(|l| {
            l.spans.iter().any(|s| {
                s.style.add_modifier.contains(Modifier::BOLD)
                    && s.style.add_modifier.contains(Modifier::UNDERLINED)
            })
        })
        .collect();
    assert!(
        heading_lines.len() > 1,
        "long h1 should wrap at narrow width"
    );
}
