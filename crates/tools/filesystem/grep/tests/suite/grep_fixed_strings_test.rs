use loopal_tool_api::{Tool, ToolContext, TypedBridge};
use loopal_tool_grep::{GrepParams, GrepTool};
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(cwd.to_path_buf(), None, Default::default());
    ToolContext::new(backend, "test")
}

fn make_file(dir: &std::path::Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).unwrap();
}

fn make_tool() -> TypedBridge<GrepTool, GrepParams> {
    TypedBridge::new(GrepTool)
}

#[tokio::test]
async fn fixed_strings_escapes_special_chars() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "f.rs", "let x = foo.bar();\nlet y = fooXbar();");
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(
            json!({"pattern": "foo.bar()", "fixed_strings": true, "output_mode": "content"}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(r.content.contains("foo.bar()"));
    assert!(!r.content.contains("fooXbar()"));
}

#[tokio::test]
async fn fixed_strings_default_false() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "f.rs", "let x = foo.bar();\nlet y = fooXbar();");
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(
            json!({"pattern": "foo.bar()", "output_mode": "content"}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(r.content.contains("foo.bar()"));
    assert!(r.content.contains("fooXbar()"));
}
