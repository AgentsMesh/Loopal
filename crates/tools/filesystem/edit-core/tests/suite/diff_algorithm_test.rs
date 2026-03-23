use loopal_edit_core::diff::{DiffOp, compute_diff, format_unified};

#[test]
fn identical_files_no_diff() {
    let lines = vec!["line1", "line2", "line3"];
    let ops = compute_diff(&lines, &lines);
    assert!(ops.iter().all(|op| matches!(op, DiffOp::Equal(_))));
}

#[test]
fn single_line_change() {
    let old = vec!["aaa", "bbb", "ccc"];
    let new = vec!["aaa", "BBB", "ccc"];
    let ops = compute_diff(&old, &new);
    assert!(ops.contains(&DiffOp::Equal("aaa".into())));
    assert!(ops.contains(&DiffOp::Delete("bbb".into())));
    assert!(ops.contains(&DiffOp::Insert("BBB".into())));
    assert!(ops.contains(&DiffOp::Equal("ccc".into())));
}

#[test]
fn added_lines() {
    let old = vec!["a", "c"];
    let new = vec!["a", "b", "c"];
    let ops = compute_diff(&old, &new);
    assert!(ops.contains(&DiffOp::Insert("b".into())));
    assert_eq!(
        ops.iter().filter(|o| matches!(o, DiffOp::Equal(_))).count(),
        2
    );
}

#[test]
fn deleted_lines() {
    let old = vec!["a", "b", "c"];
    let new = vec!["a", "c"];
    let ops = compute_diff(&old, &new);
    assert!(ops.contains(&DiffOp::Delete("b".into())));
    assert_eq!(
        ops.iter().filter(|o| matches!(o, DiffOp::Equal(_))).count(),
        2
    );
}

#[test]
fn unified_format_header() {
    let old = vec!["hello"];
    let new = vec!["world"];
    let ops = compute_diff(&old, &new);
    let output = format_unified("a.txt", "b.txt", &ops, 3);
    assert!(output.starts_with("--- a.txt\n+++ b.txt\n"));
    assert!(output.contains("@@"));
    assert!(output.contains("-hello"));
    assert!(output.contains("+world"));
}

#[test]
fn unified_format_context_lines() {
    let old = vec!["ctx1", "ctx2", "old", "ctx3", "ctx4"];
    let new = vec!["ctx1", "ctx2", "new", "ctx3", "ctx4"];
    let ops = compute_diff(&old, &new);
    let output = format_unified("a", "b", &ops, 2);
    // Context lines should appear with space prefix
    assert!(output.contains(" ctx1"));
    assert!(output.contains(" ctx2"));
    assert!(output.contains("-old"));
    assert!(output.contains("+new"));
    assert!(output.contains(" ctx3"));
    assert!(output.contains(" ctx4"));
}

#[test]
fn empty_to_nonempty() {
    let old: Vec<&str> = vec![];
    let new = vec!["new line"];
    let ops = compute_diff(&old, &new);
    assert_eq!(ops.len(), 1);
    assert!(matches!(&ops[0], DiffOp::Insert(s) if s == "new line"));
}

#[test]
fn nonempty_to_empty() {
    let old = vec!["old line"];
    let new: Vec<&str> = vec![];
    let ops = compute_diff(&old, &new);
    assert_eq!(ops.len(), 1);
    assert!(matches!(&ops[0], DiffOp::Delete(s) if s == "old line"));
}
