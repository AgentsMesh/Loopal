use loopal_config::{NetworkPolicy, ResolvedPolicy, SandboxPolicy};
use loopal_tool_api::{Tool, ToolContext, TypedBridge};
use loopal_tool_apply_patch::{ApplyPatchParams, ApplyPatchTool};
use serde_json::json;

fn make_readonly_ctx(cwd: &std::path::Path) -> ToolContext {
    let policy = ResolvedPolicy {
        policy: SandboxPolicy::ReadOnly,
        writable_paths: vec![],
        deny_write_globs: vec![],
        deny_read_globs: vec![],
        network: NetworkPolicy::default(),
    };
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        Some(policy),
        loopal_backend::ResourceLimits::default(),
        "test-session",
    );
    ToolContext::new(backend, "test")
}

fn make_tool() -> impl Tool {
    TypedBridge::<ApplyPatchTool, ApplyPatchParams>::new(ApplyPatchTool)
}

#[tokio::test]
async fn add_in_readonly_sandbox_rejected_with_permission_diagnostic() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = make_tool();
    let ctx = make_readonly_ctx(tmp.path());

    let patch = "*** Add File: new.txt\n+content\n";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(r.is_error, "ReadOnly sandbox must reject Add");
    assert!(
        r.content.to_lowercase().contains("read-only")
            || r.content.contains("permission")
            || r.content.contains("denied"),
        "expected permission/readonly diagnostic, got: {}",
        r.content
    );
    assert!(
        !tmp.path().join("new.txt").exists(),
        "no file must be created under readonly sandbox"
    );
}

#[tokio::test]
async fn update_in_readonly_sandbox_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("a.txt");
    std::fs::write(&file, "original\n").unwrap();
    let tool = make_tool();
    let ctx = make_readonly_ctx(tmp.path());

    let patch = "*** Update File: a.txt\n@@\n-original\n+changed\n";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(r.is_error, "ReadOnly sandbox must reject Update");
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "original\n",
        "file must not be mutated"
    );
}

#[tokio::test]
async fn delete_in_readonly_sandbox_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("a.txt");
    std::fs::write(&file, "data").unwrap();
    let tool = make_tool();
    let ctx = make_readonly_ctx(tmp.path());

    let patch = "*** Delete File: a.txt\n";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(r.is_error, "ReadOnly sandbox must reject Delete");
    assert!(file.exists(), "file must persist under readonly rejection");
}
