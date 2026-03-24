use loopal_tool_api::{Tool, ToolContext};
use loopal_tool_grep::GrepTool;
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(cwd.to_path_buf(), None, Default::default());
    ToolContext {
        backend,
        session_id: "test".into(),
        shared: None,
        pending_cwd_switch: Default::default(),
        memory_channel: None,
        output_tail: None,
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
                "path": "subdir",
                "output_mode": "content"
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
        .execute(
            json!({"pattern": "findable", "output_mode": "content"}),
            &ctx,
        )
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
                "glob": "[invalid"
            }),
            &ctx,
        )
        .await;

    assert!(result.is_err());
}

// --- output_mode / head_limit tests ---

#[tokio::test]
async fn test_grep_default_mode_files_with_matches() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.rs"), "hello\nhello again").unwrap();
    std::fs::write(tmp.path().join("b.rs"), "hello world").unwrap();

    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"pattern": "hello"}), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("matches"));
}

#[tokio::test]
async fn test_grep_content_mode() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("code.rs"), "fn main() {}\nfn bar() {}").unwrap();

    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"pattern": "fn", "output_mode": "content"}), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains(":1:"));
    assert!(result.content.contains("fn main"));
}

#[tokio::test]
async fn test_grep_count_mode() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("data.txt"), "aaa\nbbb\naaa\naaa").unwrap();

    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"pattern": "aaa", "output_mode": "count"}), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("3 matches across 1 files"));
}

#[tokio::test]
async fn test_grep_head_limit() {
    let tmp = tempfile::tempdir().unwrap();
    let lines: String = (0..100).map(|i| format!("match_line_{i}\n")).collect();
    std::fs::write(tmp.path().join("big.txt"), lines).unwrap();

    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "pattern": "match_line",
                "output_mode": "content",
                "head_limit": 5
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("Showing 5 of"));
    let match_lines: Vec<_> = result
        .content
        .lines()
        .filter(|l| l.contains("match_line_"))
        .collect();
    assert_eq!(match_lines.len(), 5);
}

#[tokio::test]
async fn test_grep_invalid_output_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"pattern": "test", "output_mode": "invalid"}), &ctx)
        .await;

    assert!(result.is_err());
}
