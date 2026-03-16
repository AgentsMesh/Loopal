use loopagent_types::tool::{Tool, ToolContext};
use loopagent_tools::builtin::grep::GrepTool;
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    ToolContext {
        cwd: cwd.to_path_buf(),
        session_id: "test".into(),
    }
}

#[tokio::test]
async fn test_grep_with_relative_path() {
    let tmp = tempfile::tempdir().unwrap();
    let sub = tmp.path().join("subdir");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("data.txt"), "findme here").unwrap();

    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "pattern": "findme",
                "path": "subdir"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("findme here"));
}

#[tokio::test]
async fn test_grep_pattern_too_long() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());

    let long_pattern = "a".repeat(1001);
    let result = tool
        .execute(json!({"pattern": long_pattern}), &ctx)
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("too long"));
}

#[tokio::test]
async fn test_grep_skips_binary_files() {
    let tmp = tempfile::tempdir().unwrap();
    // Write a file with invalid UTF-8 bytes
    std::fs::write(tmp.path().join("binary.bin"), [0xFF, 0xFE, 0x00, 0x01]).unwrap();
    std::fs::write(tmp.path().join("text.txt"), "findable line").unwrap();

    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"pattern": "findable"}), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("findable line"));
}

#[tokio::test]
async fn test_grep_invalid_include_glob() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "pattern": "hello",
                "include": "[invalid"
            }),
            &ctx,
        )
        .await;

    assert!(result.is_err());
}
