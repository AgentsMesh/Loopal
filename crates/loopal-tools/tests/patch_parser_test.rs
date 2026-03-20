use loopal_tools::edit::patch_parser::parse_patch;
use loopal_tools::edit::patch_types::{FileOp, HunkLine};

#[test]
fn parse_add_file() {
    let patch = "*** Add File: src/new.rs\n+fn main() {}\n";
    let ops = parse_patch(patch).unwrap();
    assert_eq!(ops.len(), 1);
    match &ops[0] {
        FileOp::Add { path, content } => {
            assert_eq!(path.to_str().unwrap(), "src/new.rs");
            assert_eq!(content, "fn main() {}\n");
        }
        _ => panic!("expected Add"),
    }
}

#[test]
fn parse_add_multiline() {
    let patch = "*** Add File: a.txt\n+line1\n+line2\n+line3\n";
    let ops = parse_patch(patch).unwrap();
    match &ops[0] {
        FileOp::Add { content, .. } => assert_eq!(content, "line1\nline2\nline3\n"),
        _ => panic!("expected Add"),
    }
}

#[test]
fn parse_delete_file() {
    let patch = "*** Delete File: old.rs\n";
    let ops = parse_patch(patch).unwrap();
    assert_eq!(ops.len(), 1);
    assert!(matches!(&ops[0], FileOp::Delete { path } if path.to_str().unwrap() == "old.rs"));
}

#[test]
fn parse_update_single_hunk() {
    let patch = "\
*** Update File: lib.rs
@@
 fn main() {
-    old_call();
+    new_call();
 }
";
    let ops = parse_patch(patch).unwrap();
    assert_eq!(ops.len(), 1);
    match &ops[0] {
        FileOp::Update { path, hunks } => {
            assert_eq!(path.to_str().unwrap(), "lib.rs");
            assert_eq!(hunks.len(), 1);
            assert!(hunks[0].line_hint.is_none());
            assert_eq!(hunks[0].lines.len(), 4);
            assert!(matches!(&hunks[0].lines[0], HunkLine::Context(s) if s == "fn main() {"));
            assert!(matches!(&hunks[0].lines[1], HunkLine::Remove(s) if s == "    old_call();"));
            assert!(matches!(&hunks[0].lines[2], HunkLine::Add(s) if s == "    new_call();"));
            assert!(matches!(&hunks[0].lines[3], HunkLine::Context(s) if s == "}"));
        }
        _ => panic!("expected Update"),
    }
}

#[test]
fn parse_update_multiple_hunks() {
    let patch = "\
*** Update File: app.rs
@@
 first
-old1
+new1
@@ 20
 second
-old2
+new2
";
    let ops = parse_patch(patch).unwrap();
    match &ops[0] {
        FileOp::Update { hunks, .. } => {
            assert_eq!(hunks.len(), 2);
            assert!(hunks[0].line_hint.is_none());
            assert_eq!(hunks[1].line_hint, Some(20));
        }
        _ => panic!("expected Update"),
    }
}

#[test]
fn parse_line_hint_with_trailing_at() {
    let patch = "\
*** Update File: x.rs
@@ 42 @@
-old
+new
";
    let ops = parse_patch(patch).unwrap();
    match &ops[0] {
        FileOp::Update { hunks, .. } => assert_eq!(hunks[0].line_hint, Some(42)),
        _ => panic!("expected Update"),
    }
}

#[test]
fn parse_empty_context_line_in_hunk() {
    let patch = "\
*** Update File: a.rs
@@
 before

-remove
+add
 after
";
    let ops = parse_patch(patch).unwrap();
    match &ops[0] {
        FileOp::Update { hunks, .. } => {
            assert_eq!(hunks[0].lines.len(), 5);
            assert!(matches!(&hunks[0].lines[1], HunkLine::Context(s) if s.is_empty()));
        }
        _ => panic!("expected Update"),
    }
}

#[test]
fn parse_multiple_file_ops() {
    let patch = "\
*** Add File: new.rs
+content

*** Update File: existing.rs
@@
-old
+new

*** Delete File: gone.rs
";
    let ops = parse_patch(patch).unwrap();
    assert_eq!(ops.len(), 3);
    assert!(matches!(&ops[0], FileOp::Add { .. }));
    assert!(matches!(&ops[1], FileOp::Update { .. }));
    assert!(matches!(&ops[2], FileOp::Delete { .. }));
}

#[test]
fn parse_error_unexpected_line() {
    let patch = "invalid line here\n";
    let err = parse_patch(patch).unwrap_err();
    assert_eq!(err.line, 1);
    assert!(err.message.contains("unexpected"));
}

#[test]
fn parse_error_update_without_hunks() {
    let patch = "*** Update File: x.rs\n*** Delete File: y.rs\n";
    let err = parse_patch(patch).unwrap_err();
    assert!(err.message.contains("no hunks"));
}

#[test]
fn parse_add_empty_file() {
    let patch = "*** Add File: empty.txt\n";
    let ops = parse_patch(patch).unwrap();
    match &ops[0] {
        FileOp::Add { content, .. } => assert!(content.is_empty()),
        _ => panic!("expected Add"),
    }
}

#[test]
fn parse_add_with_blank_plus_line() {
    // A bare '+' line means an empty line in the file content
    let patch = "*** Add File: a.txt\n+first\n+\n+third\n";
    let ops = parse_patch(patch).unwrap();
    match &ops[0] {
        FileOp::Add { content, .. } => assert_eq!(content, "first\n\nthird\n"),
        _ => panic!("expected Add"),
    }
}
