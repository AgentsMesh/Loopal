use std::sync::Arc;
use std::time::Duration;

use loopal_protocol::{
    WorkflowAttemptId, WorkflowCancelRequest, WorkflowFailureClass, WorkflowRequestId,
    WorkflowRunId, WorkflowRunState,
};

use super::journal_support::TestJournal;
use super::scheduler_support::{SpawnerEffect, coordinator, test_spawner};
use super::support::{get_request, owner, request};
use crate::workflow::scheduler::WorkflowCleanupStatus;

#[tokio::test(start_paused = true)]
async fn wall_timeout_and_deadline_share_one_preparation_abort() {
    let run_id = WorkflowRunId::new("wrun_prepare_timeout_race");
    let journal = Arc::new(TestJournal::new());
    let (spawner, control) = test_spawner();
    let (handle, task, _, _) = coordinator(
        100..120,
        [run_id.clone()],
        [WorkflowAttemptId::new("watt_prepare_timeout_race")],
        journal,
        spawner,
    );
    let owner = owner("session-prepare-timeout-race", "root");
    let mut start = request("wreq_prepare_timeout_race");
    start.spec.nodes.remove(0);
    start.spec.nodes[0].dependencies.clear();
    start.spec.limits.attempt_timeout_ms = 50;
    start.spec.limits.run_deadline_ms = 1_000;
    handle.start(owner.clone(), start).await.unwrap();
    handle
        .schedule(owner.clone(), run_id.clone())
        .await
        .unwrap();
    let SpawnerEffect::Prepare {
        response: prepare, ..
    } = control.next().await
    else {
        panic!("expected preparation")
    };

    tokio::time::advance(Duration::from_millis(50)).await;
    let SpawnerEffect::AbortPrepare {
        response: abort, ..
    } = control.next().await
    else {
        panic!("expected wall-timeout preparation abort")
    };
    assert!(prepare.is_closed());

    handle.tick(200).await.unwrap();
    control.assert_idle().await;

    let _ = abort.send(WorkflowCleanupStatus::Confirmed);
    drop(handle);
    task.abort();
    let _ = task.await;
}

#[tokio::test]
async fn timed_out_prepare_abort_cancels_stuck_task_and_fails_closed() {
    let run_id = WorkflowRunId::new("wrun_stuck_prepare");
    let journal = Arc::new(TestJournal::new());
    let (spawner, control) = test_spawner();
    let (handle, task, _, _) = coordinator(
        [100, 101, 102, 103, 104, 105, 106, 107],
        [run_id.clone()],
        [WorkflowAttemptId::new("watt_stuck_prepare")],
        journal.clone(),
        spawner,
    );
    let owner = owner("session-stuck-prepare", "root");
    let mut start = request("wreq_stuck_prepare");
    start.spec.nodes.remove(0);
    start.spec.nodes[0].dependencies.clear();
    handle.start(owner.clone(), start).await.unwrap();
    handle
        .schedule(owner.clone(), run_id.clone())
        .await
        .unwrap();
    let SpawnerEffect::Prepare {
        response: prepare, ..
    } = control.next().await
    else {
        panic!("expected preparation")
    };

    handle
        .cancel(
            owner.clone(),
            WorkflowCancelRequest {
                request_id: WorkflowRequestId::new("wreq_stuck_prepare_cancel"),
                run_id: run_id.clone(),
                reason: Some("stop now".into()),
            },
        )
        .await
        .unwrap();
    let SpawnerEffect::AbortPrepare { response, .. } = control.next().await else {
        panic!("expected preparation abort")
    };
    assert!(response.send(WorkflowCleanupStatus::TimedOut).is_ok());
    journal.wait_for_event_batches(4).await;

    assert!(prepare.is_closed());
    let run = handle
        .get(owner, get_request("wreq_stuck_prepare_get", run_id.clone()))
        .await
        .unwrap()
        .run
        .unwrap();
    assert_eq!(run.state, WorkflowRunState::Failed);
    assert_eq!(run.attempts.len(), 1);
    assert_eq!(
        run.failure.as_ref().unwrap().class,
        WorkflowFailureClass::AmbiguousExecution
    );
    assert_eq!(
        run.attempts[0].completion.as_ref().unwrap().reason,
        "workflow_cleanup_timeout"
    );
    assert_eq!(
        run.attempts[0].failure.as_ref().unwrap().class,
        WorkflowFailureClass::AmbiguousExecution
    );
    control.assert_idle().await;

    handle.shutdown().await.unwrap();
    task.await.unwrap();
}
