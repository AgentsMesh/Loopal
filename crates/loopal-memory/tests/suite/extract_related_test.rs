use loopal_memory::extract::related::normalize_related;

#[test]
fn normalizes_md_suffix_and_dedups() {
    let items = vec![
        "foo.md".to_string(),
        "[[foo]]".to_string(),
        "foo".to_string(),
        "bar.md".to_string(),
    ];
    assert_eq!(normalize_related(&items), vec!["foo", "bar"]);
}

#[test]
fn empty_input_yields_empty() {
    assert!(normalize_related(&[]).is_empty());
}

#[test]
fn skips_invalid_entries_silently() {
    let items = vec![
        "OK".to_string(),
        "valid-slug".to_string(),
        "  ".to_string(),
        "bad chars!".to_string(),
        "valid-slug".to_string(),
    ];
    assert_eq!(normalize_related(&items), vec!["valid-slug"]);
}

#[test]
fn preserves_first_seen_order() {
    let items = vec![
        "third".to_string(),
        "first.md".to_string(),
        "first".to_string(),
        "second".to_string(),
    ];
    assert_eq!(normalize_related(&items), vec!["third", "first", "second"]);
}
