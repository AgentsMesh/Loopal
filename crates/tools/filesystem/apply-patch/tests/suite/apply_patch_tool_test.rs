use loopal_tool_api::{PermissionLevel, Tool, ToolContext};
use loopal_tool_apply_patch::ApplyPatchTool;
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    ToolContext {
        session_id: "test".into(),
        shared: None,
        pending_cwd_switch: Default::default(),
        backend,
    }
}

#[test]
fn test_name_and_permission() {
    let tool = ApplyPatchTool;
    assert_eq!(tool.name(), "ApplyPatch");
    assert_eq!(tool.permission(), PermissionLevel::Supervised);
}

#[tokio::test]
async fn test_create_file() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = ApplyPatchTool;
    let ctx = make_ctx(tmp.path());

    let patch = "*** Add File: hello.txt\n+hello world\n";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(!r.is_error, "unexpected error: {}", r.content);
    assert!(r.content.contains("1 created"));

    let content = std::fs::read_to_string(tmp.path().join("hello.txt")).unwrap();
    assert_eq!(content, "hello world\n");
}

#[tokio::test]
async fn test_update_file() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("lib.rs");
    std::fs::write(&file, "fn main() {\n    old_call();\n}\n").unwrap();

    let tool = ApplyPatchTool;
    let ctx = make_ctx(tmp.path());

    let patch = "\
*** Update File: lib.rs
@@
 fn main() {
-    old_call();
+    new_call();
 }
";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(!r.is_error, "unexpected error: {}", r.content);
    assert!(r.content.contains("1 updated"));

    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "fn main() {\n    new_call();\n}\n");
}

#[tokio::test]
async fn test_delete_file() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("old.txt");
    std::fs::write(&file, "bye").unwrap();

    let tool = ApplyPatchTool;
    let ctx = make_ctx(tmp.path());

    let r = tool.execute(json!({"patch": "*** Delete File: old.txt\n"}), &ctx).await.unwrap();
    assert!(!r.is_error, "unexpected error: {}", r.content);
    assert!(r.content.contains("1 deleted"));
    assert!(!file.exists());
}

#[tokio::test]
async fn test_multi_file_atomic() {
    let tmp = tempfile::tempdir().unwrap();
    let existing = tmp.path().join("a.rs");
    std::fs::write(&existing, "old\n").unwrap();
    let to_delete = tmp.path().join("b.rs");
    std::fs::write(&to_delete, "bye").unwrap();

    let tool = ApplyPatchTool;
    let ctx = make_ctx(tmp.path());

    let patch = "\
*** Add File: c.rs
+new file

*** Update File: a.rs
@@
-old
+updated

*** Delete File: b.rs
";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(!r.is_error, "unexpected error: {}", r.content);
    assert!(r.content.contains("1 updated"));
    assert!(r.content.contains("1 created"));
    assert!(r.content.contains("1 deleted"));

    assert_eq!(std::fs::read_to_string(tmp.path().join("c.rs")).unwrap(), "new file\n");
    assert_eq!(std::fs::read_to_string(&existing).unwrap(), "updated\n");
    assert!(!to_delete.exists());
}

#[tokio::test]
async fn test_trim_whitespace_fallback() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("x.rs");
    std::fs::write(&file, "  hello  \n").unwrap();

    let tool = ApplyPatchTool;
    let ctx = make_ctx(tmp.path());

    let patch = "\
*** Update File: x.rs
@@
-  hello
+  world
";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(!r.is_error, "unexpected error: {}", r.content);
    assert!(r.content.contains("1 updated"));
}

#[tokio::test]
async fn test_line_hint_disambiguation() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("dup.rs");
    std::fs::write(&file, "marker\nAAA\nBBB\nmarker\nCCC\n").unwrap();

    let tool = ApplyPatchTool;
    let ctx = make_ctx(tmp.path());

    let patch = "\
*** Update File: dup.rs
@@ 4
-marker
+REPLACED
";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(!r.is_error, "unexpected error: {}", r.content);

    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "marker\nAAA\nBBB\nREPLACED\nCCC\n");
}
