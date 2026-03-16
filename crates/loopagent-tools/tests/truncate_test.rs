use loopagent_tools::truncate::truncate_output;

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
    // L17: byte_count + line_bytes > max_bytes triggers truncation
    let input = "short\nthis is a longer line\nthird line";
    let result = truncate_output(input, 100, 10);
    assert!(result.contains("truncated"));
    assert!(result.starts_with("short"));
}

#[test]
fn test_single_line_no_truncation() {
    // L25: line_count > 0 is false for first line
    let result = truncate_output("hello", 10, 1000);
    assert_eq!(result, "hello");
}

#[test]
fn test_exactly_at_line_limit() {
    // Two lines with max_lines=2, should not truncate
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
