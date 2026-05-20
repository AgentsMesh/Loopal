use loopal_tool_api::{Tool, ToolContext, TypedBridge};
use loopal_tool_multi_edit::{MultiEditParams, MultiEditTool};
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
        "test-session",
    );
    ToolContext::new(backend, "test")
}

fn make_tool() -> impl Tool {
    TypedBridge::<MultiEditTool, MultiEditParams>::new(MultiEditTool)
}

#[tokio::test]
async fn cascade_edit_2_applies_to_edit_1_output() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("a.txt");
    std::fs::write(&file, "ALPHA").unwrap();
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let r = tool
        .execute(
            json!({
                "file_path": "a.txt",
                "edits": [
                    { "old_string": "ALPHA", "new_string": "BETA" },
                    { "old_string": "BETA",  "new_string": "GAMMA" },
                ]
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(!r.is_error, "cascade should succeed: {}", r.content);
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "GAMMA",
        "edit 2 must apply to edit 1's output"
    );
}

#[tokio::test]
async fn rejects_multi_match_without_unique() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("a.txt");
    std::fs::write(&file, "x x x").unwrap();
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let r = tool
        .execute(
            json!({
                "file_path": "a.txt",
                "edits": [{ "old_string": "x", "new_string": "Y" }]
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        r.is_error,
        "multiple matches must be rejected: {}",
        r.content
    );
    assert!(r.content.contains("found 3 times"), "got: {}", r.content);
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "x x x",
        "file must not be mutated on rejection"
    );
}

#[tokio::test]
async fn rejects_empty_edits_array() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "data").unwrap();
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let r = tool
        .execute(json!({"file_path": "a.txt", "edits": []}), &ctx)
        .await
        .unwrap();
    assert!(r.is_error, "empty edits array must be rejected");
    assert!(
        r.content.contains("must not be empty"),
        "got: {}",
        r.content
    );
}

#[tokio::test]
async fn nonexistent_file_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let r = tool
        .execute(
            json!({
                "file_path": "missing.txt",
                "edits": [{ "old_string": "a", "new_string": "b" }]
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(r.is_error, "nonexistent file must yield error");
}
