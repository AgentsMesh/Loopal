use loopal_tool_api::{Tool, ToolContext, TypedBridge};
use loopal_tool_apply_patch::{ApplyPatchParams, ApplyPatchTool};
use loopal_tool_invocation::ToolResultMetadata;
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
        "test-session",
    );
    ToolContext::new(backend, "test")
}

fn make_tool() -> impl Tool {
    TypedBridge::<ApplyPatchTool, ApplyPatchParams>::new(ApplyPatchTool)
}

fn resolved_metadata_paths(ctx: &ToolContext, relative_paths: &[&str]) -> Vec<String> {
    relative_paths
        .iter()
        .map(|path| {
            ctx.backend
                .resolve_path(path, true)
                .unwrap()
                .as_str()
                .into_owned()
        })
        .collect()
}

#[tokio::test]
async fn update_after_delete_same_file_in_batch_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("x.txt");
    std::fs::write(&file, "original\n").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let patch = "\
*** Delete File: x.txt

*** Update File: x.txt
@@
-original
+changed
";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(r.is_error, "expected reject, got: {}", r.content);
    assert!(
        r.content.contains("deleted earlier in this patch"),
        "expected diagnostic about prior delete, got: {}",
        r.content
    );
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "original\n",
        "file must not be mutated"
    );
}

#[tokio::test]
async fn update_after_update_same_file_in_batch_rejected_v1() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("x.txt");
    std::fs::write(&file, "a\nb\n").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let patch = "\
*** Update File: x.txt
@@
-a
+A

*** Update File: x.txt
@@
-b
+B
";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(r.is_error, "v1 must reject sequential update of same file");
    assert!(
        r.content.contains("added/modified earlier in this patch"),
        "got: {}",
        r.content
    );
}

#[tokio::test]
async fn delete_after_delete_same_file_in_batch_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("x.txt");
    std::fs::write(&file, "bye").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let patch = "\
*** Delete File: x.txt

*** Delete File: x.txt
";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(r.is_error, "double-delete must be rejected");
    assert!(
        r.content.contains("already deleted in this patch"),
        "got: {}",
        r.content
    );
}

#[tokio::test]
async fn delete_after_add_same_file_in_batch_collapses_to_noop() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("new.txt");
    assert!(!target.exists());

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let patch = "\
*** Add File: new.txt
+content

*** Delete File: new.txt
";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(
        r.is_error,
        "Add-then-Delete: plan allows, but commit Delete fails because file was never created"
    );
    assert!(!target.exists(), "file must not exist after rejected batch");
}

#[tokio::test]
async fn delete_then_add_reports_updated_kind() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("x.txt");
    std::fs::write(&file, "old").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let patch = "\
*** Delete File: x.txt

*** Add File: x.txt
+new
";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(!r.is_error, "should succeed: {}", r.content);
    assert!(
        r.content.contains("updated"),
        "Delete-then-Add of pre-existing file must report 'updated' \
         (was_existing=true from initial_exists), got: {}",
        r.content
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "new\n");
}

#[tokio::test]
async fn mixed_ops_success_message_includes_all_counts() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("update.txt"), "old\n").unwrap();
    std::fs::write(tmp.path().join("delete.txt"), "bye").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());
    let expected_paths =
        resolved_metadata_paths(&ctx, &["created.txt", "update.txt", "delete.txt"]);

    let patch = "\
*** Add File: created.txt
+brand new

*** Update File: update.txt
@@
-old
+new

*** Delete File: delete.txt
";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(!r.is_error, "should succeed: {}", r.content);
    assert!(r.content.contains("1 updated"), "got: {}", r.content);
    assert!(r.content.contains("1 created"), "got: {}", r.content);
    assert!(r.content.contains("1 deleted"), "got: {}", r.content);
    assert!(r.content.starts_with("Applied: "), "got: {}", r.content);
    let Some(ToolResultMetadata::ModifiedFiles { paths }) = r.metadata else {
        panic!("successful batch must report its actual modified paths");
    };
    assert_eq!(paths.len(), 3);
    for expected in expected_paths {
        assert!(
            paths.contains(&expected),
            "missing {expected:?} in {paths:?}"
        );
    }
}

#[tokio::test]
async fn commit_failure_reports_applied_list_and_failed_index() {
    let tmp = tempfile::tempdir().unwrap();
    // Pre-create a *file* at "blocker" path; subsequent Add of "blocker/file.txt"
    // passes plan (file_info on blocker/file.txt returns ENOTDIR, treated as
    // not-existing) but fails in commit (write_file's create_dir_all(blocker)
    // fails because blocker is already a file).
    std::fs::write(tmp.path().join("blocker"), "not a dir").unwrap();

    let tool = make_tool();
    let ctx = make_ctx(tmp.path());
    let expected_paths = resolved_metadata_paths(&ctx, &["a.txt", "b.txt"]);

    let patch = "\
*** Add File: a.txt
+first

*** Add File: b.txt
+second

*** Add File: blocker/file.txt
+third
";
    let r = tool.execute(json!({"patch": patch}), &ctx).await.unwrap();
    assert!(
        r.is_error,
        "commit-stage write must fail; got: {}",
        r.content
    );
    assert!(
        r.content.contains("failed at op"),
        "should report which op failed; got: {}",
        r.content
    );
    assert!(
        r.content.contains("a.txt") && r.content.contains("b.txt"),
        "should list already-applied files (a.txt, b.txt) for LLM to know what landed; got: {}",
        r.content
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(),
        "first\n",
        "first Add applied"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("b.txt")).unwrap(),
        "second\n",
        "second Add applied"
    );
    assert!(
        !tmp.path().join("blocker/file.txt").exists(),
        "third Add never written"
    );
    let Some(ToolResultMetadata::ModifiedFiles { paths }) = r.metadata else {
        panic!("partial failure must report paths that reached disk");
    };
    assert_eq!(paths, expected_paths);
}
