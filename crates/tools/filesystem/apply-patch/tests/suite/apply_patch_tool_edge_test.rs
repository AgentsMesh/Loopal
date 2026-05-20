use loopal_tool_api::{Tool, ToolContext, TypedBridge};
use loopal_tool_apply_patch::{ApplyPatchParams, ApplyPatchTool};
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
    TypedBridge::<ApplyPatchTool, ApplyPatchParams>::new(ApplyPatchTool)
}

#[tokio::test]
async fn test_omission_in_add() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let patch = "*** Add File: x.rs\n+fn main() {\n+    // ... existing code\n+}\n";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(r.is_error);
    assert!(r.content.contains("omission"));
}

#[tokio::test]
async fn test_omission_in_update_add_lines() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("a.rs");
    std::fs::write(&file, "fn main() {}\n").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let patch = "\
*** Update File: a.rs
@@
-fn main() {}
+fn main() {
+    // ... rest of code
+}
";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(r.is_error);
    assert!(r.content.contains("omission"));
}

#[tokio::test]
async fn test_path_traversal_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = make_tool();
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

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let patch = "*** Add File: dup.txt\n+new\n";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(r.is_error);
    assert!(r.content.contains("already exists"));
}

#[tokio::test]
async fn test_delete_missing_file_error() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let r = tool
        .execute(json!({"patch": "*** Delete File: nope.txt\n"}), &ctx)
        .await
        .unwrap();
    assert!(r.is_error);
    assert!(r.content.contains("does not exist"));
}

#[tokio::test]
async fn test_hunk_not_found_error() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.rs"), "hello\n").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let patch = "\
*** Update File: a.rs
@@
-nonexistent line
+replacement
";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(r.is_error);
    assert!(r.content.contains("hunk not found"));
}

#[tokio::test]
async fn test_empty_patch_error() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let r = tool.execute(json!({"patch": ""}), &ctx).await.unwrap();
    assert!(r.is_error);
    assert!(r.content.contains("no file operations"));
}

#[tokio::test]
async fn test_missing_patch_param_error() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let r = tool.execute(json!({}), &ctx).await;
    assert!(r.is_err());
}

#[tokio::test]
async fn test_parse_error_forwarded() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let r = tool
        .execute(json!({"patch": "garbage input"}), &ctx)
        .await
        .unwrap();
    assert!(r.is_error);
    assert!(r.content.contains("parse error"));
    assert!(
        r.content.contains("line 1"),
        "parse error must include line number for LLM to locate problem; got: {}",
        r.content
    );
}

#[tokio::test]
async fn test_precheck_rejects_wire_ref_marker_in_patch() {
    let tool = make_tool();

    let patch = "\
*** Add File: secret.txt
+token=<secret_ref:api_key>
";
    let rejection = tool.precheck(&json!({"patch": patch}));
    assert!(
        rejection.is_some(),
        "precheck must reject patch carrying <secret_ref:...>"
    );
}
