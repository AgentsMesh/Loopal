//! Tests pinning down per-agent isolation: each agent process owns its
//! own stores, so mutations on one `AgentShared` must never leak into
//! another. These tests run two `AgentShared` instances inside the
//! same OS process to simulate the cross-process boundary; the contract
//! holds because each instance is constructed independently.
//!
//! Also covers `SchedulerHandle::Drop` cancelling its token — critical
//! for sub-agent shutdown to avoid orphan tick tasks.

use std::sync::Arc;

use loopal_agent::shared::SchedulerHandle;
use loopal_scheduler::CronScheduler;
use loopal_test_support::TestFixture;
use loopal_test_support::agent_ctx::agent_tool_context;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn two_agents_have_isolated_task_stores() {
    let fixture_a = TestFixture::new();
    let fixture_b = TestFixture::new();
    let (_ctx_a, shared_a) = agent_tool_context(&fixture_a);
    let (_ctx_b, shared_b) = agent_tool_context(&fixture_b);

    shared_a
        .task_store
        .create("agent A task", "only A should see this")
        .await;

    let snap_a = shared_a.snapshot_state().await;
    let snap_b = shared_b.snapshot_state().await;

    assert_eq!(snap_a.tasks.len(), 1);
    assert_eq!(snap_a.tasks[0].subject, "agent A task");
    assert!(
        snap_b.tasks.is_empty(),
        "agent B must not see agent A's task"
    );
}

#[tokio::test]
async fn two_agents_have_isolated_schedulers() {
    let fixture_a = TestFixture::new();
    let fixture_b = TestFixture::new();
    let (_ctx_a, shared_a) = agent_tool_context(&fixture_a);
    let (_ctx_b, shared_b) = agent_tool_context(&fixture_b);

    shared_a
        .scheduler_handle
        .scheduler
        .add("0 9 * * *", "agent A cron", true, false)
        .await
        .expect("add cron on A");

    let snap_a = shared_a.snapshot_state().await;
    let snap_b = shared_b.snapshot_state().await;

    assert_eq!(snap_a.crons.len(), 1);
    assert_eq!(snap_a.crons[0].prompt, "agent A cron");
    assert!(
        snap_b.crons.is_empty(),
        "agent B must not see agent A's cron"
    );
}

#[tokio::test]
async fn two_agents_have_isolated_bg_stores() {
    let fixture_a = TestFixture::new();
    let fixture_b = TestFixture::new();
    let (_ctx_a, shared_a) = agent_tool_context(&fixture_a);
    let (_ctx_b, shared_b) = agent_tool_context(&fixture_b);

    shared_a
        .kernel
        .bg_store()
        .register_proxy("bg_a_1".into(), "background on A".into());

    let snap_a = shared_a.snapshot_state().await;
    let snap_b = shared_b.snapshot_state().await;

    assert_eq!(snap_a.bg_tasks.len(), 1);
    assert_eq!(snap_a.bg_tasks[0].id, "bg_a_1");
    assert!(
        snap_b.bg_tasks.is_empty(),
        "agent B must not see agent A's background task"
    );
}

#[tokio::test]
async fn scheduler_handle_drop_cancels_token() {
    let scheduler = Arc::new(CronScheduler::new());
    let token = CancellationToken::new();

    let handle = SchedulerHandle::new(scheduler, token.clone());
    assert!(!token.is_cancelled());

    drop(handle);
    assert!(
        token.is_cancelled(),
        "SchedulerHandle::Drop must cancel its token to stop the tick loop"
    );
}
