use std::sync::Arc;

use loopal_protocol::{
    WorkflowAttemptId, WorkflowCancelRequest, WorkflowRequestId, WorkflowRunId, WorkflowRunState,
};

use super::journal_support::TestJournal;
use super::scheduler_support::{SpawnerEffect, coordinator, prepared_worker, test_spawner};
use super::support::{get_request, owner, request};
use crate::workflow::scheduler::WorkflowCleanupStatus;

#[tokio::test]
async fn timed_out_late_shutdown_fails_closed_without_retry() {
    let run_id = WorkflowRunId::new("wrun_prepare_shutdown_timeout");
    let journal = Arc::new(TestJournal::new());
    let (spawner, control) = test_spawner();
    let (handle, task, _, _) = coordinator(
        [100, 101, 102, 103, 104, 105, 106, 107],
        [run_id.clone()],
        [WorkflowAttemptId::new("watt_prepare_shutdown_timeout")],
        journal.clone(),
        spawner,
    );
    let owner = owner("session-prepare-shutdown-timeout", "root");
    let mut start = request("wreq_prepare_shutdown_timeout");
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
                request_id: WorkflowRequestId::new("wreq_prepare_shutdown_timeout_cancel"),
                run_id: run_id.clone(),
                reason: Some("stop now".into()),
            },
        )
        .await
        .unwrap();
    let SpawnerEffect::AbortPrepare {
        response: abort, ..
    } = control.next().await
    else {
        panic!("expected preparation abort")
    };

    let (worker, outcome) = prepared_worker("retry-late-worker", 37);
    assert!(prepare.send(Ok(worker)).is_ok());
    let SpawnerEffect::Shutdown {
        execution,
        response,
        ..
    } = control.next().await
    else {
        panic!("expected first late worker containment")
    };
    assert_eq!(execution.connection_generation, 37);
    assert!(abort.send(WorkflowCleanupStatus::TimedOut).is_ok());
    assert!(response.send(WorkflowCleanupStatus::TimedOut).is_ok());
    assert!(outcome.is_closed());

    // A late lease is already under one exact containment supervisor. Once
    // that supervisor times out, the coordinator must fail closed and release
    // the pending slot instead of spawning another supervisor on every tick.
    handle.tick(106).await.unwrap();
    handle.tick(107).await.unwrap();
    journal.wait_for_event_batches(4).await;

    let run = handle
        .get(
            owner,
            get_request("wreq_prepare_shutdown_timeout_get", run_id),
        )
        .await
        .unwrap()
        .run
        .unwrap();
    assert_eq!(run.state, WorkflowRunState::Failed);
    assert_eq!(run.attempts.len(), 1);
    assert_eq!(
        run.attempts[0].failure.as_ref().unwrap().class,
        loopal_protocol::WorkflowFailureClass::AmbiguousExecution
    );
    assert_eq!(
        run.attempts[0].completion.as_ref().unwrap().reason,
        "workflow_cleanup_timeout"
    );
    control.assert_idle().await;
    drop(handle);
    task.await.unwrap();
}
