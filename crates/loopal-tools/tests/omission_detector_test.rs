use loopal_tools::edit::omission_detector::detect_omissions;

#[test]
fn test_detect_rest_of_code() {
    let text = "fn main() {\n    // ... rest of code\n}";
    let found = detect_omissions(text);
    assert!(!found.is_empty());
}

#[test]
fn test_no_omission() {
    let text = "fn main() {\n    println!(\"hello\");\n}";
    assert!(detect_omissions(text).is_empty());
}

#[test]
fn test_detect_existing_code() {
    let text = "// ... existing code\nlet x = 1;";
    let found = detect_omissions(text);
    assert!(found.iter().any(|p| p.contains("existing code")));
}

#[test]
fn test_detect_remaining() {
    let text = "# ... remaining\npass";
    assert!(!detect_omissions(text).is_empty());
}

#[test]
fn test_detect_case_insensitive() {
    let text = "// ... REST OF CODE";
    let found = detect_omissions(text);
    assert!(!found.is_empty());
}

#[test]
fn test_multiple_patterns() {
    let text = "// ... rest\n// ... existing code";
    let found = detect_omissions(text);
    assert!(found.len() >= 2);
}
