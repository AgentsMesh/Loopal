use std::sync::Arc;

use loopal_protocol::{WorkflowAttemptId, WorkflowFailureClass, WorkflowRunId, WorkflowRunState};

use super::journal_support::TestJournal;
use super::scheduler_support::{SpawnerEffect, coordinator, test_spawner};
use super::support::{get_request, owner, request};
use crate::workflow::scheduler::WorkflowCleanupStatus;

#[tokio::test]
async fn confirmed_timeout_cleanup_dispatches_retry() {
    let run_id = WorkflowRunId::new("wrun_prepare_lifetime");
    let journal = Arc::new(TestJournal::new());
    let (spawner, control) = test_spawner();
    let (handle, task, _, ids) = coordinator(
        100..120,
        [run_id.clone()],
        [
            WorkflowAttemptId::new("watt_prepare_lifetime_first"),
            WorkflowAttemptId::new("watt_prepare_lifetime_retry"),
        ],
        journal,
        spawner,
    );
    let owner = owner("session-prepare-lifetime", "root");
    let mut start = request("wreq_prepare_lifetime");
    make_short_lived(&mut start);
    handle.start(owner.clone(), start).await.unwrap();
    handle.schedule(owner, run_id).await.unwrap();
    let SpawnerEffect::Prepare {
        response: first, ..
    } = control.next().await
    else {
        panic!("expected first preparation")
    };
    let SpawnerEffect::AbortPrepare {
        response: cleanup, ..
    } = control.next().await
    else {
        panic!("expected bounded preparation cleanup")
    };
    assert!(first.is_closed());
    cleanup.send(WorkflowCleanupStatus::Confirmed).unwrap();
    let SpawnerEffect::Prepare { request: retry, .. } = control.next().await else {
        panic!("expected bounded preparation retry")
    };

    assert_eq!(
        retry.causation.attempt_id.as_str(),
        "watt_prepare_lifetime_retry"
    );
    assert_eq!(ids.attempt_calls(), 2);
    task.abort();
    let _ = task.await;
}

#[tokio::test]
async fn uncertain_timeout_cleanup_fails_without_retry() {
    let run_id = WorkflowRunId::new("wrun_prepare_cleanup_timeout");
    let journal = Arc::new(TestJournal::new());
    let (spawner, control) = test_spawner();
    let (handle, task, _, _) = coordinator(
        200..220,
        [run_id.clone()],
        [WorkflowAttemptId::new("watt_prepare_cleanup_timeout")],
        journal.clone(),
        spawner,
    );
    let owner = owner("session-prepare-cleanup-timeout", "root");
    let mut start = request("wreq_prepare_cleanup_timeout");
    make_short_lived(&mut start);
    handle.start(owner.clone(), start).await.unwrap();
    handle
        .schedule(owner.clone(), run_id.clone())
        .await
        .unwrap();
    let SpawnerEffect::Prepare {
        response: first, ..
    } = control.next().await
    else {
        panic!("expected preparation")
    };
    let SpawnerEffect::AbortPrepare {
        response: cleanup, ..
    } = control.next().await
    else {
        panic!("expected bounded preparation cleanup")
    };
    assert!(first.is_closed());
    cleanup.send(WorkflowCleanupStatus::TimedOut).unwrap();
    journal.wait_for_event_batches(3).await;

    let run = handle
        .get(
            owner,
            get_request("wreq_prepare_cleanup_timeout_get", run_id),
        )
        .await
        .unwrap()
        .run
        .unwrap();
    assert_eq!(run.state, WorkflowRunState::Failed);
    assert_eq!(
        run.failure.as_ref().unwrap().class,
        WorkflowFailureClass::AmbiguousExecution
    );
    assert_eq!(
        run.attempts[0].completion.as_ref().unwrap().reason,
        "workflow_cleanup_timeout"
    );
    control.assert_idle().await;
    task.abort();
    let _ = task.await;
}

fn make_short_lived(request: &mut loopal_protocol::WorkflowStartRequest) {
    request.spec.nodes.remove(0);
    request.spec.nodes[0].dependencies.clear();
    request.spec.limits.attempt_timeout_ms = 50;
    request.spec.limits.run_deadline_ms = 1_000;
}
