use std::sync::Arc;

use loopal_protocol::{WorkflowAttemptId, WorkflowCancelRequest, WorkflowRequestId, WorkflowRunId};

use super::journal_support::TestJournal;
use super::scheduler_support::{SpawnerEffect, coordinator, prepared_worker, test_spawner};
use super::support::{get_request, owner, request};
use crate::workflow::WorkflowCoordinatorError;
use crate::workflow::scheduler::WorkflowCleanupStatus;

#[tokio::test]
async fn journal_poison_contains_a_successful_preparation_callback() {
    let run_id = WorkflowRunId::new("wrun_prepare_poison");
    let journal = Arc::new(TestJournal::new());
    let (spawner, control) = test_spawner();
    let (handle, task, _, _) = coordinator(
        [100, 101, 102, 103, 104, 105],
        [run_id.clone()],
        [WorkflowAttemptId::new("watt_prepare_poison")],
        journal.clone(),
        spawner,
    );
    let owner = owner("session-prepare-poison", "root");
    let mut start = request("wreq_prepare_poison");
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

    journal.push_append_error(WorkflowCoordinatorError::JournalUnavailable);
    assert_eq!(
        handle
            .cancel(
                owner.clone(),
                WorkflowCancelRequest {
                    request_id: WorkflowRequestId::new("wreq_prepare_poison_cancel"),
                    run_id: run_id.clone(),
                    reason: Some("poison journal".into()),
                },
            )
            .await,
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
    let SpawnerEffect::AbortPrepare {
        response: abort, ..
    } = control.next().await
    else {
        panic!("expected preparation abort after poisoning")
    };

    let (worker, outcome) = prepared_worker("poisoned-late-worker", 23);
    assert!(prepare.send(Ok(worker)).is_ok());
    let SpawnerEffect::Shutdown {
        execution,
        response,
        ..
    } = control.next().await
    else {
        panic!("expected poisoned worker containment")
    };
    assert_eq!(execution.address.agent, "poisoned-late-worker");
    assert_eq!(execution.connection_generation, 23);
    assert!(response.send(WorkflowCleanupStatus::Confirmed).is_ok());
    assert!(outcome.is_closed());
    let _ = abort.send(WorkflowCleanupStatus::Confirmed);

    assert_eq!(
        handle
            .get(owner, get_request("wreq_prepare_poison_get", run_id),)
            .await,
        Err(WorkflowCoordinatorError::OwnerPoisoned)
    );
    drop(handle);
    task.await.unwrap();
}
