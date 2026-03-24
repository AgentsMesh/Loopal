use loopal_tool_api::{Tool, ToolContext};
use loopal_tool_write::WriteTool;
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    ToolContext {
        session_id: "test".into(),
        shared: None,
        pending_cwd_switch: Default::default(),
        memory_channel: None,
        output_tail: None,
        backend,
    }
}

#[tokio::test]
async fn test_write_relative_path_existing_file_within_cwd() {
    let tmp = tempfile::tempdir().unwrap();
    let canon = tmp.path().canonicalize().unwrap();
    let file = canon.join("existing.txt");
    std::fs::write(&file, "original").unwrap();

    let tool = WriteTool;
    let ctx = make_ctx(&canon);

    let result = tool
        .execute(
            json!({
                "file_path": "existing.txt",
                "content": "overwritten"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "overwritten");
}

#[tokio::test]
async fn test_write_relative_new_file_parent_doesnt_exist() {
    let tmp = tempfile::tempdir().unwrap();
    let canon = tmp.path().canonicalize().unwrap();

    let tool = WriteTool;
    let ctx = make_ctx(&canon);

    let result = tool
        .execute(
            json!({
                "file_path": "deep/nested/new_dir/file.txt",
                "content": "new nested file"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let content = std::fs::read_to_string(canon.join("deep/nested/new_dir/file.txt")).unwrap();
    assert_eq!(content, "new nested file");
}
