use loopal_view_state::conversation::format_server_tool_content;
use serde_json::json;

#[test]
fn direct_text_field_extracted() {
    let v = json!({"text": "hello world"});
    assert_eq!(format_server_tool_content(&v), "hello world");
}

#[test]
fn array_of_text_objects_joined_by_newline() {
    let v = json!([
        {"text": "line one"},
        {"text": "line two"},
        {"text": "line three"}
    ]);
    assert_eq!(
        format_server_tool_content(&v),
        "line one\nline two\nline three"
    );
}

#[test]
fn empty_array_falls_back_to_json() {
    let v = json!([]);
    let formatted = format_server_tool_content(&v);
    assert_eq!(formatted, "[]");
}

#[test]
fn array_with_mixed_objects_keeps_text_only() {
    let v = json!([
        {"text": "keep"},
        {"other": "drop"},
        {"text": "also keep"}
    ]);
    assert_eq!(format_server_tool_content(&v), "keep\nalso keep");
}

#[test]
fn array_without_any_text_falls_back_to_json() {
    let v = json!([{"other": "x"}]);
    let formatted = format_server_tool_content(&v);
    assert!(formatted.contains("\"other\""));
}

#[test]
fn null_falls_back_to_json() {
    let v = json!(null);
    assert_eq!(format_server_tool_content(&v), "null");
}

#[test]
fn nested_object_without_text_falls_back_to_json() {
    let v = json!({"nested": {"text": "deep"}});
    let formatted = format_server_tool_content(&v);
    assert!(formatted.contains("nested"));
    assert!(formatted.contains("deep"));
}
