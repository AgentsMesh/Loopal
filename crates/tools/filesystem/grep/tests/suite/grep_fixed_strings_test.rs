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
    }
}

fn make_file(dir: &std::path::Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).unwrap();
}

#[tokio::test]
async fn fixed_strings_escapes_special_chars() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "f.rs", "let x = foo.bar();\nlet y = fooXbar();");
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(
            json!({"pattern": "foo.bar()", "fixed_strings": true, "output_mode": "content"}),
            &ctx,
        )
        .await
        .unwrap();
    // Should match the literal "foo.bar()" but not "fooXbar()"
    assert!(r.content.contains("foo.bar()"));
    assert!(!r.content.contains("fooXbar()"));
}

#[tokio::test]
async fn fixed_strings_default_false() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "f.rs", "let x = foo.bar();\nlet y = fooXbar();");
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(
            json!({"pattern": "foo.bar()", "output_mode": "content"}),
            &ctx,
        )
        .await
        .unwrap();
    // Without fixed_strings, "." matches any char → both lines match
    assert!(r.content.contains("foo.bar()"));
    assert!(r.content.contains("fooXbar()"));
}
