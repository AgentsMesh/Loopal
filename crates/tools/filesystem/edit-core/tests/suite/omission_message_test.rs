use loopal_edit_core::omission_message::format_omission_error;

#[test]
fn formats_with_category_and_omissions() {
    let msg = format_omission_error("content", &["// ... rest".into()]);
    assert!(msg.contains("Omission detected in content"));
    assert!(msg.contains("// ... rest"));
    assert!(msg.contains("complete content"));
}

#[test]
fn joins_multiple_omissions_with_comma() {
    let msg = format_omission_error("new_string", &["// ... rest".into(), "/* ... */".into()]);
    assert!(msg.contains("// ... rest, /* ... */"));
}

#[test]
fn empty_omissions_still_well_formed() {
    let msg = format_omission_error("content", &[]);
    assert!(msg.contains("Omission detected in content"));
}
