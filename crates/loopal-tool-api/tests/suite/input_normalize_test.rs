use serde_json::json;

use loopal_tool_api::input_normalize::strip_empty_optionals;

#[test]
fn empty_string_on_optional_field_is_removed() {
    let mut input = json!({"file_path": "/tmp/x", "description": ""});
    strip_empty_optionals(&mut input, &["file_path"]);
    assert_eq!(input, json!({"file_path": "/tmp/x"}));
}

#[test]
fn empty_string_on_required_field_is_preserved() {
    let mut input = json!({"command": "", "timeout": 30});
    strip_empty_optionals(&mut input, &["command"]);
    assert_eq!(input, json!({"command": "", "timeout": 30}));
}

#[test]
fn non_empty_optional_string_is_preserved() {
    let mut input = json!({"file_path": "/tmp/x", "description": "hello"});
    strip_empty_optionals(&mut input, &["file_path"]);
    assert_eq!(
        input,
        json!({"file_path": "/tmp/x", "description": "hello"})
    );
}

#[test]
fn null_optional_field_is_preserved() {
    let mut input = json!({"file_path": "/tmp/x", "offset": null});
    strip_empty_optionals(&mut input, &["file_path"]);
    assert_eq!(input, json!({"file_path": "/tmp/x", "offset": null}));
}

#[test]
fn non_string_fields_are_untouched() {
    let mut input = json!({"command": "ls", "timeout": 0, "run_in_background": false});
    strip_empty_optionals(&mut input, &["command"]);
    assert_eq!(
        input,
        json!({"command": "ls", "timeout": 0, "run_in_background": false})
    );
}

#[test]
fn non_object_input_is_noop() {
    let mut input = json!("just a string");
    strip_empty_optionals(&mut input, &[]);
    assert_eq!(input, json!("just a string"));
}

#[test]
fn multiple_empty_optionals_all_removed() {
    let mut input = json!({"file_path": "/x", "offset": "", "limit": "", "pages": ""});
    strip_empty_optionals(&mut input, &["file_path"]);
    assert_eq!(input, json!({"file_path": "/x"}));
}
