use loopal_tool_api::{Tool, ToolContext};
use loopal_tool_glob::GlobTool;
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend =
        loopal_backend::LocalBackend::new(cwd.to_path_buf(), None, Default::default());
    ToolContext { backend, session_id: "test".into(), shared: None, pending_cwd_switch: Default::default() }
}

fn make_file(dir: &std::path::Path, name: &str) {
    std::fs::write(dir.join(name), "content").unwrap();
}

#[tokio::test]
async fn type_filter_rust_only() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "main.rs");
    make_file(tmp.path(), "script.py");
    make_file(tmp.path(), "lib.rs");
    let tool = GlobTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"pattern": "**/*", "type": "rust"}), &ctx)
        .await
        .unwrap();
    assert!(r.content.contains("main.rs"));
    assert!(r.content.contains("lib.rs"));
    assert!(!r.content.contains("script.py"));
}

#[tokio::test]
async fn type_filter_unknown_empty() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "code.rs");
    let tool = GlobTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"pattern": "**/*", "type": "brainfuck"}), &ctx)
        .await
        .unwrap();
    assert!(r.content.contains("No files matched"));
}

#[tokio::test]
async fn type_with_glob_combined() {
    let tmp = tempfile::tempdir().unwrap();
    let sub = tmp.path().join("src");
    std::fs::create_dir_all(&sub).unwrap();
    make_file(&sub, "main.rs");
    make_file(&sub, "helper.py");
    make_file(tmp.path(), "root.rs");
    let tool = GlobTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"pattern": "src/**/*", "type": "rust"}), &ctx)
        .await
        .unwrap();
    assert!(r.content.contains("main.rs"));
    assert!(!r.content.contains("helper.py"));
    assert!(!r.content.contains("root.rs"));
}
