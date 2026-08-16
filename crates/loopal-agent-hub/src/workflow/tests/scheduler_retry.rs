use std::sync::Arc;

use loopal_protocol::{
    AgentCompletion, WorkflowAttemptFailure, WorkflowAttemptId, WorkflowFailureClass, WorkflowRunId,
};

use super::journal_support::TestJournal;
use super::scheduler_support::{SpawnerEffect, coordinator, test_spawner};
use super::support::{owner, request};
use crate::workflow::scheduler::WorkflowSpawnFailure;

#[tokio::test]
async fn pending_timeout_failure_automatically_dispatches_retry() {
    let run_id = WorkflowRunId::new("wrun_pending_retry");
    let journal = Arc::new(TestJournal::new());
    let (spawner, control) = test_spawner();
    let (handle, task, _, ids) = coordinator(
        [100, 101, 102, 103, 104, 105, 106, 107],
        [run_id.clone()],
        [
            WorkflowAttemptId::new("watt_pending_first"),
            WorkflowAttemptId::new("watt_pending_retry"),
        ],
        journal.clone(),
        spawner,
    );
    let owner = owner("session-pending-retry", "root");
    let mut start = request("wreq_pending_retry");
    start.spec.limits.max_attempts = 3;
    handle.start(owner.clone(), start).await.unwrap();
    handle.schedule(owner.clone(), run_id).await.unwrap();

    let SpawnerEffect::Prepare {
        request: first,
        response: first_response,
    } = control.next().await
    else {
        panic!("expected first prepare")
    };
    assert_eq!(first.causation.attempt_id.as_str(), "watt_pending_first");
    handle.tick(30_104).await.unwrap();
    let SpawnerEffect::AbortPrepare {
        causation,
        response: abort,
        ..
    } = control.next().await
    else {
        panic!("expected first preparation abort")
    };
    assert_eq!(causation.attempt_id.as_str(), "watt_pending_first");
    assert!(
        first_response
            .send(Err(WorkflowSpawnFailure {
                completion: AgentCompletion::new("prepare_timeout", None),
                failure: WorkflowAttemptFailure {
                    class: WorkflowFailureClass::TransientBeforeExecution,
                    reason: "prepare completed after timeout".into(),
                },
            }))
            .is_ok()
    );
    let _ = abort.send(crate::workflow::scheduler::WorkflowCleanupStatus::Confirmed);

    let SpawnerEffect::Prepare { request: retry, .. } = control.next().await else {
        panic!("expected automatic retry prepare")
    };
    assert_eq!(retry.causation.attempt_id.as_str(), "watt_pending_retry");
    assert_eq!(ids.attempt_calls(), 2);
    drop(handle);
    task.abort();
    let _ = task.await;
}
