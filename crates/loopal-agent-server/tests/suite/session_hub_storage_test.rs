//! Tests for `SessionHub` singleton storage behavior — root commit,
//! lazy init, root-mismatch error, and concurrent safety.

use loopal_agent_server::testing::{SessionHub, SessionHubError};
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn first_call_commits_root_subsequent_calls_return_same_instance() {
    let dir = tempdir().unwrap();
    let hub = SessionHub::default();
    let a = hub.cron_storage(dir.path()).await.unwrap();
    let b = hub.cron_storage(dir.path()).await.unwrap();
    assert!(
        Arc::ptr_eq(&a, &b),
        "second call must return the same Arc — singleton invariant"
    );
}

#[tokio::test]
async fn task_storage_is_singleton_too() {
    let dir = tempdir().unwrap();
    let hub = SessionHub::default();
    let a = hub.task_storage(dir.path()).await.unwrap();
    let b = hub.task_storage(dir.path()).await.unwrap();
    assert!(Arc::ptr_eq(&a, &b));
}

#[tokio::test]
async fn cron_and_task_storage_share_root_commitment() {
    // Once cron_storage commits a root, task_storage with the same
    // root must succeed.
    let dir = tempdir().unwrap();
    let hub = SessionHub::default();
    hub.cron_storage(dir.path()).await.unwrap();
    hub.task_storage(dir.path()).await.unwrap();
}

#[tokio::test]
async fn cron_storage_with_different_root_returns_root_mismatch_err() {
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();
    let hub = SessionHub::default();
    hub.cron_storage(dir1.path()).await.unwrap();
    let result = hub.cron_storage(dir2.path()).await;
    let err = match result {
        Ok(_) => panic!("expected RootMismatch"),
        Err(e) => e,
    };
    if let SessionHubError::RootMismatch {
        committed,
        requested,
    } = err
    {
        assert_eq!(committed, dir1.path());
        assert_eq!(requested, dir2.path());
    } else {
        panic!("expected RootMismatch variant");
    }
}

#[tokio::test]
async fn task_storage_after_cron_with_different_root_errs() {
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();
    let hub = SessionHub::default();
    hub.cron_storage(dir1.path()).await.unwrap();
    let result = hub.task_storage(dir2.path()).await;
    assert!(matches!(result, Err(SessionHubError::RootMismatch { .. })));
}

#[tokio::test]
async fn root_mismatch_does_not_poison_the_committed_storage() {
    // After a failed mismatch, subsequent calls with the original root
    // must still succeed — the failed attempt is a no-op.
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();
    let hub = SessionHub::default();
    let a = hub.cron_storage(dir1.path()).await.unwrap();
    let _ = hub.cron_storage(dir2.path()).await; // err, ignored
    let b = hub.cron_storage(dir1.path()).await.unwrap();
    assert!(
        Arc::ptr_eq(&a, &b),
        "valid roots must keep returning the original singleton"
    );
}

#[tokio::test]
async fn concurrent_first_calls_yield_a_single_instance() {
    // Two tasks racing the lazy-init path must converge on the same
    // Arc — `commit_storage_root` + `cron_storage` together serialize
    // through `tokio::sync::Mutex` so the second loser sees the
    // already-initialized storage.
    let dir = tempdir().unwrap();
    let hub = Arc::new(SessionHub::default());
    let h1 = {
        let hub = hub.clone();
        let dir = dir.path().to_path_buf();
        tokio::spawn(async move { hub.cron_storage(&dir).await.unwrap() })
    };
    let h2 = {
        let hub = hub.clone();
        let dir = dir.path().to_path_buf();
        tokio::spawn(async move { hub.cron_storage(&dir).await.unwrap() })
    };
    let a = h1.await.unwrap();
    let b = h2.await.unwrap();
    assert!(
        Arc::ptr_eq(&a, &b),
        "concurrent first-callers must agree on the singleton"
    );
}
