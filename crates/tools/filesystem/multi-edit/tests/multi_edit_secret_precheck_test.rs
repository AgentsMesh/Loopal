use loopal_tool_api::{Tool, TypedBridge};
use loopal_tool_multi_edit::{MultiEditParams, MultiEditTool};
use serde_json::json;

fn make_tool() -> TypedBridge<MultiEditTool, MultiEditParams> {
    TypedBridge::new(MultiEditTool)
}

#[test]
fn precheck_rejects_secret_in_file_path() {
    let tool = make_tool();
    let rejection = tool.precheck(&json!({
        "file_path": "<secret_ref:path>",
        "edits": [{ "old_string": "a", "new_string": "b" }]
    }));
    assert!(rejection.is_some());
}

#[test]
fn precheck_rejects_secret_in_any_edit_old_string() {
    let tool = make_tool();
    let rejection = tool.precheck(&json!({
        "file_path": "a.txt",
        "edits": [
            { "old_string": "clean", "new_string": "ok" },
            { "old_string": "<secret_ref:api_key>", "new_string": "x" },
        ]
    }));
    assert!(rejection.is_some());
}

#[test]
fn precheck_rejects_secret_in_any_edit_new_string() {
    let tool = make_tool();
    let rejection = tool.precheck(&json!({
        "file_path": "a.txt",
        "edits": [
            { "old_string": "a", "new_string": "<secret_ref:value>" }
        ]
    }));
    assert!(rejection.is_some());
}

#[test]
fn precheck_passes_when_no_secret_anywhere() {
    let tool = make_tool();
    let rejection = tool.precheck(&json!({
        "file_path": "a.txt",
        "edits": [
            { "old_string": "a", "new_string": "A" },
            { "old_string": "b", "new_string": "B" }
        ]
    }));
    assert!(rejection.is_none());
}
