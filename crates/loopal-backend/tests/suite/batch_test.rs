use loopal_backend::batch::apply_batch;
use loopal_tool_api::{AppliedKind, BatchOp, BatchWriteKind, ResolvedPath};

fn write_op(path: std::path::PathBuf, content: &str, kind: BatchWriteKind) -> BatchOp {
    BatchOp::Write {
        path: ResolvedPath::from_backend_resolved(path),
        content: content.to_string(),
        expected_kind: kind,
    }
}

fn delete_op(path: std::path::PathBuf) -> BatchOp {
    BatchOp::Delete {
        path: ResolvedPath::from_backend_resolved(path),
    }
}

#[tokio::test]
async fn apply_batch_all_success_returns_applied_list() {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("a.txt");
    let b = tmp.path().join("b.txt");

    let outcome = apply_batch(vec![
        write_op(a.clone(), "alpha", BatchWriteKind::Create),
        write_op(b.clone(), "bravo", BatchWriteKind::Create),
    ])
    .await;

    assert!(outcome.failed_at.is_none());
    assert_eq!(outcome.applied.len(), 2);
    assert_eq!(outcome.applied[0].kind, AppliedKind::Created);
    assert_eq!(std::fs::read_to_string(&a).unwrap(), "alpha");
    assert_eq!(std::fs::read_to_string(&b).unwrap(), "bravo");
}

#[tokio::test]
async fn apply_batch_reports_update_kind_when_specified() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("x.txt");
    std::fs::write(&file, "old").unwrap();

    let outcome = apply_batch(vec![write_op(file.clone(), "new", BatchWriteKind::Update)]).await;

    assert!(outcome.failed_at.is_none());
    assert_eq!(outcome.applied[0].kind, AppliedKind::Updated);
}

#[tokio::test]
async fn apply_batch_fails_fast_returning_applied_and_failed_index() {
    let tmp = tempfile::tempdir().unwrap();
    let ok = tmp.path().join("ok.txt");
    let dir_collision = tmp.path().join("collision");
    std::fs::create_dir(&dir_collision).unwrap();

    let outcome = apply_batch(vec![
        write_op(ok.clone(), "first", BatchWriteKind::Create),
        write_op(dir_collision.clone(), "second", BatchWriteKind::Create),
        write_op(
            tmp.path().join("never.txt"),
            "third",
            BatchWriteKind::Create,
        ),
    ])
    .await;

    let failed = outcome.failed_at.expect("expected fail-fast outcome");
    assert_eq!(failed.index, 1);
    assert_eq!(failed.path, dir_collision);
    assert_eq!(outcome.applied.len(), 1);
    assert_eq!(outcome.applied[0].path, ok);
    assert!(!tmp.path().join("never.txt").exists());
}

#[tokio::test]
async fn apply_batch_deletes_existing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("doomed.txt");
    std::fs::write(&file, "bye").unwrap();

    let outcome = apply_batch(vec![delete_op(file.clone())]).await;

    assert!(outcome.failed_at.is_none());
    assert_eq!(outcome.applied[0].kind, AppliedKind::Deleted);
    assert!(!file.exists());
}

#[tokio::test]
async fn apply_batch_deletes_directory_recursively() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("nested");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("inner.txt"), "data").unwrap();

    let outcome = apply_batch(vec![delete_op(dir.clone())]).await;

    assert!(outcome.failed_at.is_none());
    assert!(!dir.exists());
}

#[tokio::test]
async fn apply_batch_delete_missing_path_fails_fast() {
    let tmp = tempfile::tempdir().unwrap();
    let outcome = apply_batch(vec![delete_op(tmp.path().join("absent.txt"))]).await;

    let failed = outcome.failed_at.expect("missing file should fail");
    assert_eq!(failed.index, 0);
    assert!(outcome.applied.is_empty());
}

#[tokio::test]
async fn apply_batch_empty_input_returns_empty_outcome() {
    let outcome = apply_batch(vec![]).await;
    assert!(outcome.failed_at.is_none());
    assert!(outcome.applied.is_empty());
}
