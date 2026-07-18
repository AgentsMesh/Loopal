use loopal_edit_core::no_match_hint::no_match_hint;

#[test]
fn flags_crlf_difference() {
    let content = "line one\r\nlet total = compute();\r\nline three\n";
    let old = "let total = compute();\n"; // LF, file is CRLF
    let hint = no_match_hint(content, old);
    assert!(
        hint.contains("CRLF"),
        "should flag line-ending difference: {hint}"
    );
}

#[test]
fn flags_whitespace_difference() {
    let content = "fn main() {\n\tlet x = 1;\n}\n"; // tab-indented
    let old = "    let x = 1;"; // space-indented — matches only ignoring whitespace
    let hint = no_match_hint(content, old);
    assert!(
        hint.contains("whitespace"),
        "should flag whitespace difference: {hint}"
    );
}

#[test]
fn points_at_nearest_line() {
    let content = "alpha\nlet compute_total = 42;\nbeta\n";
    let old = "let compute_total = 99;"; // token compute_total anchors line 2
    let hint = no_match_hint(content, old);
    assert!(
        hint.contains("Nearest line is 2"),
        "should name the closest line: {hint}"
    );
    assert!(
        hint.contains("compute_total"),
        "should quote the line: {hint}"
    );
}

#[test]
fn always_nudges_a_reread() {
    let hint = no_match_hint("totally different content\n", "xyzzy_unmatched_1234");
    assert!(
        hint.to_lowercase().contains("re-read"),
        "should always nudge a re-read: {hint}"
    );
}
