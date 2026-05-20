use loopal_edit_core::hunks::{HunkError, apply_hunks_to_text};
use loopal_edit_core::patch_types::{Hunk, HunkLine};

fn hunk(line_hint: Option<usize>, lines: Vec<HunkLine>) -> Hunk {
    Hunk { line_hint, lines }
}

#[test]
fn apply_hunks_to_text_simple_replace() {
    let original = "fn main() {\n    old();\n}\n";
    let h = hunk(
        None,
        vec![
            HunkLine::Context("fn main() {".into()),
            HunkLine::Remove("    old();".into()),
            HunkLine::Add("    new();".into()),
            HunkLine::Context("}".into()),
        ],
    );
    let r = apply_hunks_to_text(original, &[h]).unwrap();
    assert_eq!(r, "fn main() {\n    new();\n}\n");
}

#[test]
fn apply_hunks_to_text_two_non_overlapping_hunks() {
    let original = "a\nb\nc\nd\ne\n";
    let h1 = hunk(
        None,
        vec![HunkLine::Remove("a".into()), HunkLine::Add("AA".into())],
    );
    let h2 = hunk(
        None,
        vec![HunkLine::Remove("d".into()), HunkLine::Add("DD".into())],
    );
    let r = apply_hunks_to_text(original, &[h1, h2]).unwrap();
    assert_eq!(r, "AA\nb\nc\nDD\ne\n");
}

#[test]
fn apply_hunks_to_text_not_found() {
    let original = "hello\n";
    let h = hunk(
        None,
        vec![
            HunkLine::Remove("missing".into()),
            HunkLine::Add("x".into()),
        ],
    );
    let err = apply_hunks_to_text(original, &[h]).unwrap_err();
    assert!(matches!(err, HunkError::NotFound { .. }));
}

#[test]
fn apply_hunks_to_text_omission_in_add() {
    let original = "fn main() {}\n";
    let h = hunk(
        None,
        vec![
            HunkLine::Remove("fn main() {}".into()),
            HunkLine::Add("fn main() {".into()),
            HunkLine::Add("    // ... rest of code".into()),
            HunkLine::Add("}".into()),
        ],
    );
    let err = apply_hunks_to_text(original, &[h]).unwrap_err();
    assert!(matches!(err, HunkError::Omission(_)));
}

#[test]
fn apply_hunks_to_text_trim_whitespace_fallback() {
    let original = "  hello  \n";
    let h = hunk(
        None,
        vec![
            HunkLine::Remove("  hello".into()),
            HunkLine::Add("  world".into()),
        ],
    );
    let r = apply_hunks_to_text(original, &[h]).unwrap();
    assert!(r.contains("world"));
}

#[test]
fn apply_hunks_to_text_rejects_overlapping_hunks() {
    let original = "a\nb\nc\nd\ne\n";
    let h1 = hunk(
        None,
        vec![
            HunkLine::Remove("a".into()),
            HunkLine::Remove("b".into()),
            HunkLine::Add("X".into()),
            HunkLine::Add("Y".into()),
        ],
    );
    let h2 = hunk(
        None,
        vec![
            HunkLine::Remove("b".into()),
            HunkLine::Remove("c".into()),
            HunkLine::Add("Z".into()),
            HunkLine::Add("W".into()),
        ],
    );
    let err = apply_hunks_to_text(original, &[h1, h2]).unwrap_err();
    assert!(matches!(err, HunkError::Overlapping { .. }));
    assert!(err.to_string().contains("overlapping hunks"));
}

#[test]
fn hunk_error_display() {
    let e = HunkError::NotFound {
        preview: vec!["foo".into(), "bar".into()],
    };
    let s = e.to_string();
    assert!(s.contains("hunk not found"));
    assert!(s.contains("foo"));
}
