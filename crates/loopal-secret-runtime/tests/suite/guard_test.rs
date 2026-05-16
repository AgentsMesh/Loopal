//! `input_contains_secret_ref` gates writes to user files (Write/Edit/MultiEdit/ApplyPatch).
//! It must walk the entire JSON tree and detect `<secret_ref:` anywhere.

use loopal_secret_runtime::{SECRET_REJECTION_MESSAGE, WIRE_REF_MARKER, input_contains_secret_ref};
use serde_json::json;

#[test]
fn detects_in_top_level_string() {
    let v = json!("here is <secret_ref:openai_key> in plain text");
    assert!(input_contains_secret_ref(&v));
}

#[test]
fn detects_in_object_value() {
    let v = json!({
        "file_path": "/tmp/x",
        "content": "API_KEY=<secret_ref:openai_key>"
    });
    assert!(input_contains_secret_ref(&v));
}

#[test]
fn detects_in_nested_array() {
    let v = json!({
        "edits": [
            { "old_string": "", "new_string": "k=<secret_ref:k>" }
        ]
    });
    assert!(input_contains_secret_ref(&v));
}

#[test]
fn detects_in_deeply_nested_structure() {
    let v = json!({
        "a": { "b": { "c": [{ "d": "leak <secret_ref:nested>" }] } }
    });
    assert!(input_contains_secret_ref(&v));
}

#[test]
fn returns_false_for_clean_input() {
    let v = json!({
        "file_path": "/tmp/x",
        "content": "no secrets here, just plain text"
    });
    assert!(!input_contains_secret_ref(&v));
}

#[test]
fn returns_false_for_empty_object() {
    assert!(!input_contains_secret_ref(&json!({})));
}

#[test]
fn returns_false_for_null_or_number_or_bool() {
    assert!(!input_contains_secret_ref(&json!(null)));
    assert!(!input_contains_secret_ref(&json!(42)));
    assert!(!input_contains_secret_ref(&json!(true)));
}

#[test]
fn detects_marker_alone_even_if_malformed() {
    // The guard is a precheck — any occurrence of the marker prefix is enough
    // to reject; we don't require well-formed `<secret_ref:NAME>` here.
    let v = json!("garbled <secret_ref: thing");
    assert!(input_contains_secret_ref(&v));
}

#[test]
fn wire_marker_constant_matches_expected_string() {
    assert_eq!(WIRE_REF_MARKER, "<secret_ref:");
}

#[test]
fn rejection_message_names_alternative() {
    // Make sure the rejection message points users at Bash env injection,
    // which is the supported path for tools that legitimately need secrets.
    assert!(SECRET_REJECTION_MESSAGE.contains("Bash"));
    assert!(SECRET_REJECTION_MESSAGE.contains("env"));
    assert!(SECRET_REJECTION_MESSAGE.contains("<secret_ref:"));
}
