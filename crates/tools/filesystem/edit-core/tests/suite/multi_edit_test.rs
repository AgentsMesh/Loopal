use loopal_edit_core::multi_edit::{MultiEditError, MultiEditItem, apply_multi_edits};

fn item(old: &str, new: &str) -> MultiEditItem {
    MultiEditItem {
        old_string: old.into(),
        new_string: new.into(),
    }
}

#[test]
fn apply_multi_edits_single_replace() {
    let r = apply_multi_edits("hello world", &[item("world", "rust")]).unwrap();
    assert_eq!(r.content, "hello rust");
    assert_eq!(r.applied, 1);
}

#[test]
fn apply_multi_edits_sequential() {
    let r =
        apply_multi_edits("a b c d", &[item("a", "A"), item("b", "B"), item("c", "C")]).unwrap();
    assert_eq!(r.content, "A B C d");
    assert_eq!(r.applied, 3);
}

#[test]
fn apply_multi_edits_rejects_when_not_found() {
    let e = apply_multi_edits("hello", &[item("missing", "x")]).unwrap_err();
    match e {
        MultiEditError::NotFound { index } => assert_eq!(index, 0),
        _ => panic!("expected NotFound"),
    }
}

#[test]
fn apply_multi_edits_rejects_when_multiple_matches() {
    let e = apply_multi_edits("a a a", &[item("a", "b")]).unwrap_err();
    match e {
        MultiEditError::MultipleMatches { index, count } => {
            assert_eq!(index, 0);
            assert_eq!(count, 3);
        }
        _ => panic!("expected MultipleMatches"),
    }
}

#[test]
fn apply_multi_edits_reports_failing_edit_index() {
    let e = apply_multi_edits(
        "alpha beta gamma",
        &[
            item("alpha", "ALPHA"),
            item("missing", "x"),
            item("beta", "BETA"),
        ],
    )
    .unwrap_err();
    match e {
        MultiEditError::NotFound { index } => assert_eq!(index, 1),
        _ => panic!("expected NotFound at index 1"),
    }
}

#[test]
fn apply_multi_edits_empty_edits_succeeds() {
    let r = apply_multi_edits("unchanged", &[]).unwrap();
    assert_eq!(r.content, "unchanged");
    assert_eq!(r.applied, 0);
}

#[test]
fn multi_edit_error_display_messages() {
    let e1 = MultiEditError::NotFound { index: 2 };
    assert!(e1.to_string().contains("edit 2"));
    assert!(e1.to_string().contains("not found"));

    let e2 = MultiEditError::MultipleMatches { index: 0, count: 5 };
    assert!(e2.to_string().contains("found 5 times"));
}
