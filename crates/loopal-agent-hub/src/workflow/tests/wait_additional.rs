use loopal_protocol::{
    QualifiedAddress, WorkflowRunId, WorkflowRunSnapshot, WorkflowRunState, WorkflowWaitRequest,
    WorkflowWaitStatus,
};
use tokio::sync::watch;

use super::super::WorkflowCoordinatorError;
use super::support::spec;

fn request(after_revision: u64, timeout_ms: u64) -> WorkflowWaitRequest {
    WorkflowWaitRequest {
        request_id: loopal_protocol::WorkflowRequestId::new("wreq_wait_additional"),
        run_id: WorkflowRunId::new("wrun_wait_additional"),
        after_revision,
        timeout_ms,
    }
}

fn snapshot(revision: u64, state: WorkflowRunState) -> WorkflowRunSnapshot {
    let mut snapshot = WorkflowRunSnapshot::planned(
        WorkflowRunId::new("wrun_wait_additional"),
        QualifiedAddress::local("root"),
        spec(),
        1,
    );
    snapshot.revision = revision;
    snapshot.state = state;
    snapshot
}

#[tokio::test]
async fn wait_returns_immediate_terminal_and_newer_revision_snapshots() {
    let terminal = snapshot(1, WorkflowRunState::Cancelled);
    let (_sender, receiver) = watch::channel(terminal.clone());
    let terminal_response = super::super::wait::wait(Some(receiver), request(1, 1))
        .await
        .unwrap();
    assert_eq!(terminal_response.status, WorkflowWaitStatus::Terminal);
    assert_eq!(terminal_response.run, Some(terminal));

    let changed = snapshot(2, WorkflowRunState::Running);
    let (_sender, receiver) = watch::channel(changed.clone());
    let changed_response = super::super::wait::wait(Some(receiver), request(1, 1))
        .await
        .unwrap();
    assert_eq!(changed_response.status, WorkflowWaitStatus::Changed);
    assert_eq!(changed_response.run, Some(changed));
}

#[tokio::test(start_paused = true)]
async fn wait_times_out_and_reports_closed_revision_streams() {
    let pending = snapshot(1, WorkflowRunState::Running);
    let (sender, receiver) = watch::channel(pending.clone());
    let wait = tokio::spawn(super::super::wait::wait(Some(receiver), request(1, 50)));
    tokio::time::advance(std::time::Duration::from_millis(50)).await;
    let timed_out = wait.await.unwrap().unwrap();
    assert_eq!(timed_out.status, WorkflowWaitStatus::TimedOut);
    assert!(timed_out.run.is_none());

    let (_sender, receiver) = watch::channel(pending);
    drop(_sender);
    assert_eq!(
        super::super::wait::wait(Some(receiver), request(1, 50)).await,
        Err(WorkflowCoordinatorError::Unavailable)
    );
    drop(sender);
}

#[tokio::test]
async fn wait_unblocks_when_the_subscription_publishes_a_new_revision() {
    let (sender, receiver) = watch::channel(snapshot(1, WorkflowRunState::Running));
    let waiter = tokio::spawn(super::super::wait::wait(Some(receiver), request(1, 500)));
    tokio::task::yield_now().await;
    sender.send_replace(snapshot(2, WorkflowRunState::Running));

    let response = waiter.await.unwrap().unwrap();
    assert_eq!(response.status, WorkflowWaitStatus::Changed);
    assert_eq!(response.run.unwrap().revision, 2);
}
