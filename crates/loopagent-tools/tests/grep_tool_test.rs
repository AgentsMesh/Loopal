use loopagent_types::permission::PermissionLevel;
use loopagent_types::tool::{Tool, ToolContext};
use loopagent_tools::builtin::grep::GrepTool;
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    ToolContext {
        cwd: cwd.to_path_buf(),
        session_id: "test".into(),
    }
}

#[test]
fn test_grep_name() {
    let tool = GrepTool;
    assert_eq!(tool.name(), "Grep");
}

#[test]
fn test_grep_description() {
    let tool = GrepTool;
    let desc = tool.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("regex"));
}

#[test]
fn test_grep_permission() {
    let tool = GrepTool;
    assert_eq!(tool.permission(), PermissionLevel::ReadOnly);
}

#[test]
fn test_grep_parameters_schema() {
    let tool = GrepTool;
    let schema = tool.parameters_schema();
    assert_eq!(schema["type"], "object");
    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&json!("pattern")));
    assert!(schema["properties"]["pattern"].is_object());
    assert!(schema["properties"]["path"].is_object());
    assert!(schema["properties"]["include"].is_object());
}

#[tokio::test]
async fn test_grep_matching_pattern_in_file() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("hello.txt"),
        "hello world\ngoodbye world\nhello again",
    )
    .unwrap();

    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"pattern": "hello"}), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("hello world"));
    assert!(result.content.contains("hello again"));
    // Should include line numbers
    assert!(result.content.contains(":1:"));
    assert!(result.content.contains(":3:"));
}

#[tokio::test]
async fn test_grep_no_matches() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("file.txt"), "nothing here").unwrap();

    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"pattern": "missing"}), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("No matches found"));
}

#[tokio::test]
async fn test_grep_regex_pattern() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("code.rs"),
        "fn main() {}\nfn helper() {}\nstruct Foo;",
    )
    .unwrap();

    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"pattern": "fn \\w+\\("}), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("fn main()"));
    assert!(result.content.contains("fn helper()"));
    assert!(!result.content.contains("struct Foo"));
}

#[tokio::test]
async fn test_grep_missing_pattern_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());

    let result = tool.execute(json!({}), &ctx).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_grep_invalid_regex_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"pattern": "(unclosed"}), &ctx)
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_grep_with_include_glob_filter() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("code.rs"), "hello rust").unwrap();
    std::fs::write(tmp.path().join("readme.md"), "hello markdown").unwrap();

    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "pattern": "hello",
                "include": "*.rs"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("hello rust"));
    assert!(!result.content.contains("hello markdown"));
}

#[tokio::test]
async fn test_grep_with_explicit_file_path() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("single.txt");
    std::fs::write(&file, "line alpha\nline beta\nline gamma").unwrap();

    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "pattern": "beta",
                "path": file.to_str().unwrap()
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("line beta"));
    assert!(!result.content.contains("alpha"));
}
