use loopal_tool_api::truncate::{extract_overflow_path, truncate_output, truncate_tail};
use loopal_tool_api::truncate_middle::truncate_middle;

#[test]
fn test_no_truncation() {
    let input = "line1\nline2\nline3";
    assert_eq!(truncate_output(input, 100, 10000), input);
}

#[test]
fn test_truncate_by_lines() {
    let input = "a\nb\nc\nd\ne";
    let result = truncate_output(input, 2, 10000);
    assert!(result.contains("truncated"));
    assert!(result.starts_with("a\nb"));
}

#[test]
fn test_empty() {
    assert_eq!(truncate_output("", 10, 100), "");
}

#[test]
fn test_truncate_by_bytes() {
    let input = "short\nthis is a longer line\nthird line";
    let result = truncate_output(input, 100, 10);
    assert!(result.contains("truncated"));
    assert!(result.starts_with("short"));
}

#[test]
fn test_single_line_no_truncation() {
    let result = truncate_output("hello", 10, 1000);
    assert_eq!(result, "hello");
}

#[test]
fn test_exactly_at_line_limit() {
    let input = "a\nb";
    let result = truncate_output(input, 2, 10000);
    assert_eq!(result, "a\nb");
}

#[test]
fn test_truncate_reports_remaining_bytes() {
    let input = "line1\nline2\nline3\nline4\nline5";
    let result = truncate_output(input, 2, 10000);
    assert!(result.contains("truncated"));
    assert!(result.contains("3 lines"));
    assert!(result.contains("bytes omitted"));
}

#[test]
fn truncate_middle_no_truncation_passthrough() {
    let input = "a\nb\nc";
    assert_eq!(truncate_middle(input, 100, 10000, 50), input);
}

#[test]
fn truncate_middle_inserts_marker() {
    let lines: Vec<String> = (0..200).map(|i| format!("line{i}")).collect();
    let input = lines.join("\n");
    let result = truncate_middle(&input, 50, 10000, 50);
    assert!(result.contains("[middle truncated:"));
    assert!(result.contains("lines, "));
    assert!(result.contains("bytes omitted"));
    assert!(result.starts_with("line0"));
    assert!(result.ends_with("line199"));
}

#[test]
fn truncate_middle_head_ratio_60_keeps_more_head() {
    let lines: Vec<String> = (0..200).map(|i| format!("L{i:03}")).collect();
    let input = lines.join("\n");
    let r60 = truncate_middle(&input, 50, 10000, 60);
    let head_60: Vec<&str> = r60.lines().take_while(|l| !l.starts_with("...")).collect();
    let r40 = truncate_middle(&input, 50, 10000, 40);
    let head_40: Vec<&str> = r40.lines().take_while(|l| !l.starts_with("...")).collect();
    assert!(head_60.len() > head_40.len());
}

#[test]
fn truncate_middle_clamps_ratio_below_10() {
    let lines: Vec<String> = (0..200).map(|i| format!("L{i:03}")).collect();
    let input = lines.join("\n");
    let r0 = truncate_middle(&input, 100, 100000, 0);
    let r10 = truncate_middle(&input, 100, 100000, 10);
    assert_eq!(r0, r10);
}

#[test]
fn truncate_middle_clamps_ratio_above_90() {
    let lines: Vec<String> = (0..200).map(|i| format!("L{i:03}")).collect();
    let input = lines.join("\n");
    let r100 = truncate_middle(&input, 100, 100000, 100);
    let r90 = truncate_middle(&input, 100, 100000, 90);
    assert_eq!(r100, r90);
}

#[test]
fn truncate_middle_byte_overrun_on_long_line() {
    let huge_line = "x".repeat(2000);
    let input = format!("a\n{huge_line}\nz");
    let result = truncate_middle(&input, 1000, 100, 50);
    assert!(result.contains("[middle truncated:"));
    assert!(result.len() < input.len());
}

#[test]
fn truncate_middle_force_cuts_when_no_newline_in_tail() {
    let input = "x".repeat(200_000);
    let result = truncate_middle(&input, 1000, 5000, 50);
    assert!(
        result.len() <= 10_000,
        "single-line oversize must be force-cut, got {} bytes",
        result.len()
    );
    assert!(result.contains("[middle truncated:"));
}

#[test]
fn truncate_middle_force_cut_preserves_utf8_boundary() {
    let mut input = String::new();
    for _ in 0..50_000 {
        input.push('中');
    }
    let result = truncate_middle(&input, 1000, 5000, 50);
    assert!(result.is_char_boundary(result.len()));
    assert!(result.contains("[middle truncated:"));
}

#[test]
fn truncate_middle_omits_head_prefix_when_first_line_oversize() {
    let huge_first = "x".repeat(100_000);
    let input = format!("{huge_first}\nkeep1\nkeep2\nkeep3\n");
    let result = truncate_middle(&input, 1000, 200, 50);
    assert!(
        !result.starts_with('\n'),
        "head=\"\" must not produce a leading newline before the marker; got {:?}",
        &result[..result.len().min(40)]
    );
    assert!(result.starts_with("... [middle truncated:"));
}

#[test]
fn truncate_middle_handles_all_newline_input() {
    let input = "\n".repeat(5_000);
    let result = truncate_middle(&input, 100, 1000, 50);
    assert!(result.len() <= 1500);
    assert!(result.contains("[middle truncated:"));
}

#[test]
fn truncate_middle_no_gap_with_trailing_newlines() {
    let mut input = String::new();
    for i in 0..50 {
        input.push_str(&format!("line{i}\n"));
    }
    input.push_str("\n\n\n\n\n");
    let result = truncate_middle(&input, 1000, 10000, 50);
    assert!(
        !result.contains("[middle truncated:"),
        "head+tail covers everything → no truncation marker; got: {result}"
    );
}

#[test]
fn truncate_tail_no_truncation_passthrough() {
    let input = "a\nb\nc";
    assert_eq!(truncate_tail(input, 100, 10000), input);
}

#[test]
fn truncate_tail_keeps_tail_lines() {
    let lines: Vec<String> = (0..200).map(|i| format!("line{i}")).collect();
    let input = lines.join("\n");
    let result = truncate_tail(&input, 30, 10000);
    assert!(result.starts_with("[head truncated:"));
    assert!(result.contains("lines, "));
    assert!(result.contains("bytes omitted"));
    assert!(result.ends_with("line199"));
    assert!(!result.contains("line0\n"));
}

#[test]
fn truncate_tail_byte_overrun_on_long_line() {
    let huge = "y".repeat(5000);
    let input = format!("a\nb\n{huge}\nz");
    let result = truncate_tail(&input, 100, 200);
    assert!(result.starts_with("[head truncated:"));
    assert!(result.ends_with("z"));
}

#[test]
fn extract_overflow_path_with_marker() {
    let body = "preview content here";
    let path = "/tmp/loopal/overflow/bash_stdout_1234.txt";
    let s = format!(
        "{body}\n\n\
         [Output too large for context (5.0 MB). Full output saved to: {path}]\n\
         Use the Read tool to access the complete output if needed."
    );
    let (extracted_body, extracted_path) = extract_overflow_path(&s);
    assert_eq!(extracted_body, body);
    assert_eq!(extracted_path.as_deref(), Some(path));
}

#[test]
fn extract_overflow_path_without_marker_passthrough() {
    let s = "regular output without any overflow marker";
    let (body, path) = extract_overflow_path(s);
    assert_eq!(body, s);
    assert!(path.is_none());
}

#[test]
fn extract_overflow_path_malformed_marker_passthrough() {
    let s = "preview\n\n[Output too large for context (1.0 MB). Full output saved to: /tmp/x.txt]\nIncomplete tail";
    let (body, path) = extract_overflow_path(s);
    assert_eq!(body, s);
    assert!(path.is_none());
}
