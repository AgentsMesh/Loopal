use loopal_tool_api::{Tool, TypedBridge};
use loopal_tool_edit::{EditParams, EditTool};
use serde_json::json;

fn make_tool() -> TypedBridge<EditTool, EditParams> {
    TypedBridge::new(EditTool)
}

#[test]
fn precheck_rejects_secret_in_old_string() {
    let tool = make_tool();
    let rejection = tool.precheck(&json!({
        "file_path": "a.txt",
        "old_string": "token=<secret_ref:api_key>",
        "new_string": "redacted"
    }));
    assert!(rejection.is_some());
}

#[test]
fn precheck_rejects_secret_in_new_string() {
    let tool = make_tool();
    let rejection = tool.precheck(&json!({
        "file_path": "a.txt",
        "old_string": "PLACEHOLDER",
        "new_string": "token=<secret_ref:api_key>"
    }));
    assert!(rejection.is_some());
}

#[test]
fn precheck_rejects_secret_in_file_path() {
    let tool = make_tool();
    let rejection = tool.precheck(&json!({
        "file_path": "<secret_ref:path>",
        "old_string": "a",
        "new_string": "b"
    }));
    assert!(rejection.is_some());
}

#[test]
fn precheck_passes_when_no_secret() {
    let tool = make_tool();
    let rejection = tool.precheck(&json!({
        "file_path": "a.txt",
        "old_string": "foo",
        "new_string": "bar"
    }));
    assert!(rejection.is_none());
}
