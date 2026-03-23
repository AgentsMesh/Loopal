use loopal_tui::views::progress::summarize_result;

#[test]
fn test_no_result_pending() {
    let s = summarize_result(None, "pending");
    assert!(s.is_empty());
}

#[test]
fn test_no_result_running() {
    let s = summarize_result(None, "running");
    assert!(s.is_empty());
}

#[test]
fn test_empty_result_success() {
    let s = summarize_result(Some(""), "success");
    assert_eq!(s, "done");
}

#[test]
fn test_single_short_line_shown_inline() {
    let s = summarize_result(Some("applied"), "success");
    assert_eq!(s, "applied");
}

#[test]
fn test_single_long_line_truncated_with_ellipsis() {
    let long = "a".repeat(50);
    let s = summarize_result(Some(&long), "success");
    assert!(
        s.ends_with("..."),
        "long single line should be truncated: {s}"
    );
    assert!(s.len() <= 40, "truncated result should be ≤40 chars: {s}");
}

#[test]
fn test_multiline_shows_count() {
    let content = "line1\nline2\nline3\nline4\nline5";
    let s = summarize_result(Some(content), "success");
    assert_eq!(s, "5 lines");
}

#[test]
fn test_error_shows_first_line() {
    let content = "TypeError: cannot read property\n    at foo.js:42";
    let s = summarize_result(Some(content), "error");
    assert_eq!(s, "TypeError: cannot read property");
}

#[test]
fn test_error_empty_lines_skipped() {
    let content = "\n\nActual error here";
    let s = summarize_result(Some(content), "error");
    assert_eq!(s, "Actual error here");
}
