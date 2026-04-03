use loopal_tui::markdown::render_markdown;
use ratatui::prelude::*;

fn lines_text(lines: &[Line<'_>]) -> Vec<String> {
    lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
        .collect()
}

// --- Basic table ---

#[test]
fn test_table_renders_header_and_rows() {
    let input = "| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |";
    let lines = render_markdown(input, 80);
    let texts = lines_text(&lines);
    assert!(texts.iter().any(|t| t.contains("A")), "header A");
    assert!(texts.iter().any(|t| t.contains("B")), "header B");
    assert!(texts.iter().any(|t| t.contains("1")), "cell 1");
    assert!(texts.iter().any(|t| t.contains("4")), "cell 4");
}

#[test]
fn test_table_header_separator_line() {
    let input = "| H1 | H2 |\n|---|---|\n| a | b |";
    let lines = render_markdown(input, 80);
    let texts = lines_text(&lines);
    // Should have a separator line with ─ and ┼
    assert!(texts.iter().any(|t| t.contains("─")), "separator dashes");
    assert!(texts.iter().any(|t| t.contains("┼")), "separator cross");
    assert!(texts[0].contains("┌"), "top border");
    assert!(texts.iter().any(|t| t.contains("└")), "bottom border");
}

#[test]
fn test_table_header_is_bold() {
    let input = "| Head |\n|---|\n| body |";
    let lines = render_markdown(input, 80);
    let header_span = lines
        .iter()
        .flat_map(|l| &l.spans)
        .find(|s| s.content.contains("Head"));
    assert!(header_span.is_some());
    assert!(
        header_span
            .unwrap()
            .style
            .add_modifier
            .contains(Modifier::BOLD),
        "header should be bold"
    );
}

#[test]
fn test_table_column_separator() {
    let input = "| A | B |\n|---|---|\n| 1 | 2 |";
    let lines = render_markdown(input, 80);
    let texts = lines_text(&lines);
    // Data rows should contain │ separator
    assert!(texts.iter().any(|t| t.contains("│")));
}

// --- Alignment ---

#[test]
fn test_table_right_alignment() {
    let input = "| Num |\n|---:|\n| 42 |";
    let lines = render_markdown(input, 80);
    let texts = lines_text(&lines);
    assert!(texts.iter().any(|t| t.contains("42")));
}

// --- Cell wrapping ---

#[test]
fn test_table_cell_wraps_when_narrow() {
    let input = "| H1 | H2 |\n|---|---|\n| short | This is a long cell value that overflows |\n";
    let lines = render_markdown(input, 30);
    let texts = lines_text(&lines);
    // Count data row lines (after separator) — wrapped row produces >= 2.
    let sep_idx = texts.iter().position(|t| t.contains("─┼─")).unwrap();
    let data_lines: Vec<_> = texts[sep_idx + 1..]
        .iter()
        .take_while(|t| !t.is_empty() && !t.contains("└"))
        .collect();
    assert!(
        data_lines.len() >= 2,
        "long cell should wrap into >= 2 visual lines, got {}: {:?}",
        data_lines.len(),
        data_lines,
    );
}

#[test]
fn test_table_wrapped_rows_keep_separators() {
    let input = "| A | B |\n|---|---|\n| x | This text is very long and must wrap |\n";
    let lines = render_markdown(input, 30);
    let texts = lines_text(&lines);
    let sep_idx = texts.iter().position(|t| t.contains("─┼─")).unwrap();
    // Every visual line of the data row should have the │ column separator.
    let data_lines: Vec<_> = texts[sep_idx + 1..]
        .iter()
        .take_while(|t| !t.is_empty() && !t.contains("└"))
        .collect();
    for line in &data_lines {
        assert!(line.contains("│"), "wrapped row line must keep │: {line:?}");
    }
}

#[test]
fn test_table_no_truncation_of_content() {
    // Verify all content is present (not truncated) after wrapping.
    let input = "| Col |\n|---|\n| alpha bravo charlie delta echo foxtrot |\n";
    let lines = render_markdown(input, 25);
    let full: String = lines_text(&lines).join(" ");
    for word in ["alpha", "bravo", "charlie", "delta", "echo", "foxtrot"] {
        assert!(full.contains(word), "word '{word}' must not be truncated");
    }
}
