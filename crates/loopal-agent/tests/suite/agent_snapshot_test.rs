//! Tests for `AgentShared::snapshot_state` — the per-agent state dump
//! used by `agent/state_snapshot` IPC for Hub ViewState cold-start rebuild.
//!
//! Each agent process owns its own stores; these tests pin down the
//! aggregation contract: which fields appear, what filtering is applied,
//! and what the empty case looks like. Cross-process isolation is
//! covered separately in `agent_isolation_test`.

use loopal_agent::TaskPatch;
use loopal_agent::types::TaskStatus;
use loopal_test_support::TestFixture;
use loopal_test_support::agent_ctx::agent_tool_context;

#[tokio::test]
async fn snapshot_empty_returns_all_empty() {
    let fixture = TestFixture::new();
    let (_ctx, shared) = agent_tool_context(&fixture);

    let snapshot = shared.snapshot_state().await;

    assert!(snapshot.tasks.is_empty());
    assert!(snapshot.crons.is_empty());
    assert!(snapshot.bg_tasks.is_empty());
}

#[tokio::test]
async fn snapshot_includes_pending_tasks() {
    let fixture = TestFixture::new();
    let (_ctx, shared) = agent_tool_context(&fixture);

    let task = shared
        .task_store
        .create("write tests", "verify the snapshot works")
        .await;

    let snapshot = shared.snapshot_state().await;

    assert_eq!(snapshot.tasks.len(), 1);
    assert_eq!(snapshot.tasks[0].id, task.id);
    assert_eq!(snapshot.tasks[0].subject, "write tests");
}

#[tokio::test]
async fn snapshot_includes_in_progress_tasks() {
    let fixture = TestFixture::new();
    let (_ctx, shared) = agent_tool_context(&fixture);

    let task = shared.task_store.create("running task", "doing it").await;
    shared
        .task_store
        .update(
            &task.id,
            TaskPatch {
                status: Some(TaskStatus::InProgress),
                ..TaskPatch::default()
            },
        )
        .await;

    let snapshot = shared.snapshot_state().await;

    assert_eq!(snapshot.tasks.len(), 1);
    assert_eq!(snapshot.tasks[0].id, task.id);
}

#[tokio::test]
async fn snapshot_excludes_completed_tasks() {
    let fixture = TestFixture::new();
    let (_ctx, shared) = agent_tool_context(&fixture);

    let kept = shared
        .task_store
        .create("still pending", "not done yet")
        .await;
    let done = shared.task_store.create("finished", "done").await;
    shared
        .task_store
        .update(
            &done.id,
            TaskPatch {
                status: Some(TaskStatus::Completed),
                ..TaskPatch::default()
            },
        )
        .await;

    let snapshot = shared.snapshot_state().await;

    assert_eq!(snapshot.tasks.len(), 1);
    assert_eq!(snapshot.tasks[0].id, kept.id);
}

#[tokio::test]
async fn snapshot_includes_scheduled_cron_jobs() {
    let fixture = TestFixture::new();
    let (_ctx, shared) = agent_tool_context(&fixture);

    let id = shared
        .scheduler_handle
        .scheduler
        .add("0 * * * *", "hourly check", true, false)
        .await
        .expect("add cron");

    let snapshot = shared.snapshot_state().await;

    assert_eq!(snapshot.crons.len(), 1);
    assert_eq!(snapshot.crons[0].id, id);
    assert_eq!(snapshot.crons[0].prompt, "hourly check");
    assert!(snapshot.crons[0].recurring);
}

#[tokio::test]
async fn snapshot_strips_newlines_from_task_subjects() {
    let fixture = TestFixture::new();
    let (_ctx, shared) = agent_tool_context(&fixture);

    shared
        .task_store
        .create("multi\nline\rsubject", "body")
        .await;

    let snapshot = shared.snapshot_state().await;

    // Conversion replaces `\n` with a space and strips `\r` outright;
    // avoids producing double whitespace on `\r\n` line endings.
    assert_eq!(snapshot.tasks[0].subject, "multi linesubject");
}
