use loopal_tool_api::{PermissionLevel, Tool, ToolContext};
use loopal_tool_glob::GlobTool;
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend =
        loopal_backend::LocalBackend::new(cwd.to_path_buf(), None, Default::default());
    ToolContext { backend, session_id: "test".into(), shared: None, pending_cwd_switch: Default::default(), memory_channel: None }
}

#[test]
fn test_glob_name() {
    let tool = GlobTool;
    assert_eq!(tool.name(), "Glob");
}

#[test]
fn test_glob_description() {
    let tool = GlobTool;
    let desc = tool.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("glob"));
}

#[test]
fn test_glob_permission() {
    let tool = GlobTool;
    assert_eq!(tool.permission(), PermissionLevel::ReadOnly);
}

#[test]
fn test_glob_parameters_schema() {
    let tool = GlobTool;
    let schema = tool.parameters_schema();
    assert_eq!(schema["type"], "object");
    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&json!("pattern")));
    assert!(schema["properties"]["pattern"].is_object());
    assert!(schema["properties"]["path"].is_object());
    assert!(schema["properties"]["offset"].is_object());
}

#[tokio::test]
async fn test_glob_matching_files_in_temp_dir() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("foo.rs"), "fn main() {}").unwrap();
    std::fs::write(tmp.path().join("bar.rs"), "fn bar() {}").unwrap();
    std::fs::write(tmp.path().join("readme.md"), "# Hello").unwrap();

    let tool = GlobTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"pattern": "*.rs"}), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("foo.rs"));
    assert!(result.content.contains("bar.rs"));
    assert!(!result.content.contains("readme.md"));
}

#[tokio::test]
async fn test_glob_recursive_pattern() {
    let tmp = tempfile::tempdir().unwrap();
    let sub = tmp.path().join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(tmp.path().join("top.rs"), "top").unwrap();
    std::fs::write(sub.join("nested.rs"), "nested").unwrap();

    let tool = GlobTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"pattern": "**/*.rs"}), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("top.rs"));
    assert!(result.content.contains("nested.rs"));
}

#[tokio::test]
async fn test_glob_no_matches_returns_message() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("file.txt"), "hello").unwrap();

    let tool = GlobTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"pattern": "*.rs"}), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("No files matched"));
}

#[tokio::test]
async fn test_glob_invalid_pattern_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = GlobTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"pattern": "[invalid"}), &ctx)
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_glob_missing_pattern_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = GlobTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({}), &ctx)
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_glob_with_explicit_path_absolute() {
    let tmp = tempfile::tempdir().unwrap();
    let sub = tmp.path().join("mydir");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("a.txt"), "a").unwrap();
    std::fs::write(tmp.path().join("b.txt"), "b").unwrap();

    let tool = GlobTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "pattern": "*.txt",
                "path": sub.to_str().unwrap()
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("a.txt"));
    assert!(!result.content.contains("b.txt"));
}

#[tokio::test]
async fn test_glob_with_explicit_path_relative() {
    let tmp = tempfile::tempdir().unwrap();
    let sub = tmp.path().join("mydir");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("c.txt"), "c").unwrap();

    let tool = GlobTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "pattern": "*.txt",
                "path": "mydir"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("c.txt"));
}

#[tokio::test]
async fn test_glob_empty_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = GlobTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"pattern": "*.txt"}), &ctx)
        .await
        .unwrap();

    // An empty directory has no .txt files so nothing matches
    assert!(result.content.contains("No files matched"));
}
