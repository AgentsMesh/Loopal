use loopal_tui::markdown::render_markdown;

fn lines_text(lines: &[ratatui::prelude::Line<'_>]) -> Vec<String> {
    lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
        .collect()
}

// --- Code block highlighting ---

#[test]
fn test_rust_code_block_highlighted() {
    let input = "```rust\nfn main() {\n    println!(\"hello\");\n}\n```";
    let lines = render_markdown(input, 80);
    let texts = lines_text(&lines);
    // Should contain the code content
    assert!(texts.iter().any(|t| t.contains("fn")));
    assert!(texts.iter().any(|t| t.contains("main")));
}

#[test]
fn test_code_block_not_wrapped() {
    // Long code line should NOT be word-wrapped
    let long_line = "x".repeat(200);
    let input = format!("```\n{}\n```", long_line);
    let lines = render_markdown(&input, 40);
    let texts = lines_text(&lines);
    // At least one line should contain the full 200-char string
    assert!(
        texts.iter().any(|t| t.len() >= 200),
        "code block lines should not be wrapped"
    );
}

#[test]
fn test_unknown_language_fallback() {
    let input = "```foobarxyz\nsome code\n```";
    let lines = render_markdown(input, 80);
    let texts = lines_text(&lines);
    assert!(texts.iter().any(|t| t.contains("some code")));
}

#[test]
fn test_empty_code_block() {
    let input = "```\n```";
    let lines = render_markdown(input, 80);
    // Should not panic; may produce empty or minimal lines
    assert!(!lines.is_empty());
}

// --- Language aliases ---

#[test]
fn test_python_highlighting() {
    let input = "```python\ndef hello():\n    pass\n```";
    let lines = render_markdown(input, 80);
    let texts = lines_text(&lines);
    assert!(texts.iter().any(|t| t.contains("def")));
}

#[test]
fn test_shell_alias() {
    let input = "```shell\necho hello\n```";
    let lines = render_markdown(input, 80);
    let texts = lines_text(&lines);
    assert!(texts.iter().any(|t| t.contains("echo")));
}

// --- Safety limits ---

#[test]
fn test_huge_code_block_falls_back() {
    // >512KB should fall back to plain text (no panic)
    let big = "x".repeat(600_000);
    let input = format!("```rust\n{}\n```", big);
    let lines = render_markdown(&input, 80);
    assert!(!lines.is_empty(), "should produce output even for huge code");
}

// --- Fenced with options ---

#[test]
fn test_fenced_language_with_comma_options() {
    // "rust,no_run" should extract "rust"
    let input = "```rust,no_run\nlet x = 1;\n```";
    let lines = render_markdown(input, 80);
    let texts = lines_text(&lines);
    assert!(texts.iter().any(|t| t.contains("let")));
}
