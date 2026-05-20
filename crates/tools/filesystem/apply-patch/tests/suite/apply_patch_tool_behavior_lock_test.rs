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

fn make_ctx_with_read_limit(cwd: &std::path::Path, limit: u64) -> ToolContext {
    let limits = loopal_backend::ResourceLimits {
        max_file_read_bytes: limit,
        ..loopal_backend::ResourceLimits::default()
    };
    let backend =
        loopal_backend::LocalBackend::new(cwd.to_path_buf(), None, limits, "test-session");
    ToolContext::new(backend, "test")
}

fn make_tool() -> impl Tool {
    TypedBridge::<ApplyPatchTool, ApplyPatchParams>::new(ApplyPatchTool)
}

#[test]
fn description_no_longer_promises_atomic() {
    let tool = make_tool();
    let desc = tool.description();
    assert!(
        !desc.contains("atomically"),
        "description must not claim atomicity post-refactor: {desc}"
    );
    assert!(
        desc.contains("best-effort"),
        "description should describe best-effort batch behavior: {desc}"
    );
}

#[tokio::test]
async fn rejects_overlapping_hunks() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("x.txt");
    std::fs::write(&file, "a\nb\nc\nd\ne\n").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let patch = "\
*** Update File: x.txt
@@
-a
-b
+X
+Y
@@
-b
-c
+Z
+W
";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(r.is_error, "overlapping hunks must be rejected");
    assert!(r.content.contains("overlapping"));
    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(
        content, "a\nb\nc\nd\ne\n",
        "file must not be mutated when patch is rejected"
    );
}

#[tokio::test]
async fn rejects_add_body_unprefixed_lines() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let patch = "*** Add File: x.txt\n+line_a\nmissing_plus\n+line_b\n";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(
        r.is_error,
        "parser must reject add body containing non-'+' non-empty lines"
    );
    assert!(
        !tmp.path().join("x.txt").exists(),
        "file must not be created when patch is rejected"
    );
}

#[tokio::test]
async fn delete_then_add_same_file_succeeds_via_virtual_fs() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("x.txt");
    std::fs::write(&file, "original").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let patch = "\
*** Delete File: x.txt

*** Add File: x.txt
+brand_new
";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(!r.is_error, "Delete-then-Add must succeed: {}", r.content);
    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "brand_new\n");
}

#[tokio::test]
async fn update_respects_backend_size_limit() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("big.txt");
    let big = "x".repeat(200);
    std::fs::write(&file, format!("{big}\noriginal\n")).unwrap();

    let tool = make_tool();
    let ctx = make_ctx_with_read_limit(tmp.path(), 100);

    let patch = "\
*** Update File: big.txt
@@
-original
+REPLACED
";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(r.is_error, "Update on file > read limit must be rejected");
    assert!(
        r.content.contains("too large") || r.content.contains("TooLarge"),
        "expected size-limit error, got: {}",
        r.content
    );
}
