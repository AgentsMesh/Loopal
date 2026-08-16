use std::sync::Arc;

use loopal_protocol::{
    WorkflowAttemptId, WorkflowCancelRequest, WorkflowRequestId, WorkflowRunId, WorkflowRunState,
};

use super::journal_support::TestJournal;
use super::scheduler_support::{SpawnerEffect, coordinator, test_spawner};
use super::support::{get_request, owner, request};

#[tokio::test]
async fn cancellation_waits_for_prepare_abort_confirmation() {
    let run_id = WorkflowRunId::new("wrun_prepare_cancel");
    let journal = Arc::new(TestJournal::new());
    let (spawner, control) = test_spawner();
    let (handle, task, _, _) = coordinator(
        [100, 101, 102, 103, 104, 105, 106, 107, 108],
        [run_id.clone()],
        [WorkflowAttemptId::new("watt_prepare_cancel")],
        journal.clone(),
        spawner,
    );
    let owner = owner("session-prepare-cancel", "root");
    let mut start = request("wreq_prepare_cancel");
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
                request_id: WorkflowRequestId::new("wreq_prepare_cancel_stop"),
                run_id: run_id.clone(),
                reason: Some("stop now".into()),
            },
        )
        .await
        .unwrap();
    let SpawnerEffect::AbortPrepare {
        causation,
        response,
        ..
    } = control.next().await
    else {
        panic!("expected preparation abort")
    };
    assert_eq!(causation.attempt_id.as_str(), "watt_prepare_cancel");
    assert_eq!(
        run(
            &handle,
            owner.clone(),
            run_id.clone(),
            "wreq_prepare_cancel_during",
        )
        .await
        .state,
        WorkflowRunState::Cancelling
    );
    assert!(
        response
            .send(crate::workflow::scheduler::WorkflowCleanupStatus::Confirmed)
            .is_ok()
    );
    journal.wait_for_event_batches(4).await;
    assert!(prepare.is_closed());
    control.assert_idle().await;

    assert_eq!(
        run(&handle, owner, run_id, "wreq_prepare_cancel_after")
            .await
            .state,
        WorkflowRunState::Cancelled
    );
    drop(handle);
    task.await.unwrap();
}

async fn run(
    handle: &crate::workflow::WorkflowCoordinatorHandle,
    owner: crate::workflow::WorkflowOwner,
    run_id: WorkflowRunId,
    request_id: &str,
) -> loopal_protocol::WorkflowRunSnapshot {
    handle
        .get(owner, get_request(request_id, run_id))
        .await
        .unwrap()
        .run
        .unwrap()
}
