use std::sync::Arc;

use loopal_tool_api::{FileReadTracker, Tool, ToolContext, TypedBridge};
use loopal_tool_write::{WriteParams, WriteTool};
use serde_json::json;

fn make_ctx_with_tracker(cwd: &std::path::Path, tracker: Arc<FileReadTracker>) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
        "test-session",
    );
    ToolContext::new(backend, "test").with_read_tracker(tracker)
}

fn write_tool() -> TypedBridge<WriteTool, WriteParams> {
    TypedBridge::new(WriteTool)
}

#[tokio::test]
async fn write_refuses_when_file_changed_since_read() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("shared.txt");
    std::fs::write(&file, "original content").unwrap();

    let tracker = Arc::new(FileReadTracker::new());
    let ctx = make_ctx_with_tracker(tmp.path(), tracker.clone());

    // Simulate the model having read the file (what ReadTool records).
    let resolved = ctx
        .backend
        .resolve_path(file.to_str().unwrap(), false)
        .unwrap();
    tracker.record(&resolved, "original content");

    // Someone else (user / another agent) edits the file on disk.
    std::fs::write(&file, "changed by another editor").unwrap();

    let result = write_tool()
        .execute(
            json!({"file_path": file.to_str().unwrap(), "content": "my overwrite"}),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.is_error, "a stale overwrite should be refused");
    assert!(result.content.contains("changed on disk"));
    // The other editor's content is preserved, not clobbered.
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "changed by another editor"
    );
}

#[tokio::test]
async fn write_succeeds_when_file_unchanged_since_read() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("stable.txt");
    std::fs::write(&file, "original").unwrap();

    let tracker = Arc::new(FileReadTracker::new());
    let ctx = make_ctx_with_tracker(tmp.path(), tracker.clone());
    let resolved = ctx
        .backend
        .resolve_path(file.to_str().unwrap(), false)
        .unwrap();
    tracker.record(&resolved, "original");

    let result = write_tool()
        .execute(
            json!({"file_path": file.to_str().unwrap(), "content": "updated"}),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "updated");
}

#[tokio::test]
async fn write_allows_overwrite_of_unread_file() {
    // No prior read recorded → not the clobber case; blind writes still work.
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("fresh.txt");
    std::fs::write(&file, "existing").unwrap();

    let ctx = make_ctx_with_tracker(tmp.path(), Arc::new(FileReadTracker::new()));

    let result = write_tool()
        .execute(
            json!({"file_path": file.to_str().unwrap(), "content": "blind"}),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "blind");
}
