use loopal_tool_api::{Tool, ToolContext};
use loopal_tool_apply_patch::ApplyPatchTool;
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
        pending_cwd_switch: Default::default(), memory_channel: None,
        backend,
    }
}

#[tokio::test]
async fn test_omission_in_add() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = ApplyPatchTool;
    let ctx = make_ctx(tmp.path());

    let patch = "*** Add File: x.rs\n+fn main() {\n+    // ... existing code\n+}\n";
    let r = tool.execute(json!({"patch": patch}), &ctx).await;
    assert!(r.is_err());
}

#[tokio::test]
async fn test_omission_in_update_add_lines() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("a.rs");
    std::fs::write(&file, "fn main() {}\n").unwrap();

    let tool = ApplyPatchTool;
    let ctx = make_ctx(tmp.path());

    let patch = "\
*** Update File: a.rs
@@
-fn main() {}
+fn main() {
+    // ... rest of code
+}
";
    let r = tool.execute(json!({"patch": patch}), &ctx).await;
    assert!(r.is_err());
}

#[tokio::test]
async fn test_path_traversal_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = ApplyPatchTool;
    let ctx = make_ctx(tmp.path());

    let patch = "*** Add File: ../escape.txt\n+evil\n";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(r.is_error);
    assert!(
        r.content.contains("path escapes working directory")
            || r.content.contains("write to path outside"),
        "unexpected error message: {}",
        r.content
    );
}

#[tokio::test]
async fn test_add_existing_file_error() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("dup.txt"), "exists").unwrap();

    let tool = ApplyPatchTool;
    let ctx = make_ctx(tmp.path());

    let patch = "*** Add File: dup.txt\n+new\n";
    let r = tool.execute(json!({"patch": patch}), &ctx).await;
    assert!(r.is_err());
}

#[tokio::test]
async fn test_delete_missing_file_error() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = ApplyPatchTool;
    let ctx = make_ctx(tmp.path());

    let r = tool.execute(json!({"patch": "*** Delete File: nope.txt\n"}), &ctx).await;
    assert!(r.is_err());
}

#[tokio::test]
async fn test_hunk_not_found_error() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.rs"), "hello\n").unwrap();

    let tool = ApplyPatchTool;
    let ctx = make_ctx(tmp.path());

    let patch = "\
*** Update File: a.rs
@@
-nonexistent line
+replacement
";
    let r = tool.execute(json!({"patch": patch}), &ctx).await;
    assert!(r.is_err());
}

#[tokio::test]
async fn test_empty_patch_error() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = ApplyPatchTool;
    let ctx = make_ctx(tmp.path());

    let r = tool.execute(json!({"patch": ""}), &ctx).await.unwrap();
    assert!(r.is_error);
    assert!(r.content.contains("no file operations"));
}

#[tokio::test]
async fn test_missing_patch_param_error() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = ApplyPatchTool;
    let ctx = make_ctx(tmp.path());

    let r = tool.execute(json!({}), &ctx).await;
    assert!(r.is_err());
}

#[tokio::test]
async fn test_parse_error_forwarded() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = ApplyPatchTool;
    let ctx = make_ctx(tmp.path());

    let r = tool.execute(json!({"patch": "garbage input"}), &ctx).await;
    assert!(r.is_err());
}
