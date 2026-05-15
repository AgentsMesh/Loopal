use loopal_tool_api::{Tool, ToolContext, TypedBridge};
use loopal_tool_glob::{GlobParams, GlobTool};
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(cwd.to_path_buf(), None, Default::default());
    ToolContext::new(backend, "test")
}

fn make_tool() -> TypedBridge<GlobTool, GlobParams> {
    TypedBridge::new(GlobTool)
}

#[tokio::test]
async fn test_glob_output_format_includes_stats() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.rs"), "").unwrap();
    std::fs::write(tmp.path().join("b.rs"), "").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"pattern": "*.rs"}), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.starts_with("Found 2 files. Showing 1-2:"));
}

#[tokio::test]
async fn test_glob_pagination_with_offset() {
    let tmp = tempfile::tempdir().unwrap();
    for i in 0..5 {
        std::fs::write(tmp.path().join(format!("file{i}.txt")), "").unwrap();
    }

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let r1 = tool
        .execute(json!({"pattern": "*.txt", "limit": 2, "offset": 0}), &ctx)
        .await
        .unwrap();
    assert!(r1.content.contains("Found 5 files. Showing 1-2:"));
    assert!(r1.content.contains("Use offset=2"));

    let r2 = tool
        .execute(json!({"pattern": "*.txt", "limit": 2, "offset": 2}), &ctx)
        .await
        .unwrap();
    assert!(r2.content.contains("Showing 3-4:"));
    assert!(r2.content.contains("Use offset=4"));

    let r3 = tool
        .execute(json!({"pattern": "*.txt", "limit": 2, "offset": 4}), &ctx)
        .await
        .unwrap();
    assert!(r3.content.contains("Showing 5-5:"));
    assert!(!r3.content.contains("Use offset="));
}

#[tokio::test]
async fn test_glob_default_limit_is_100() {
    let tmp = tempfile::tempdir().unwrap();
    for i in 0..105 {
        std::fs::write(tmp.path().join(format!("f{i:03}.txt")), "").unwrap();
    }

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"pattern": "*.txt"}), &ctx)
        .await
        .unwrap();

    assert!(result.content.contains("Found 105 files. Showing 1-100:"));
    assert!(result.content.contains("Use offset=100"));
}
