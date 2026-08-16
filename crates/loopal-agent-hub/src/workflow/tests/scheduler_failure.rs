use std::sync::Arc;

use loopal_protocol::{WorkflowAttemptId, WorkflowCancelRequest, WorkflowRequestId, WorkflowRunId};

use super::journal_support::TestJournal;
use super::scheduler_support::{SpawnerEffect, coordinator, prepared_worker, test_spawner};
use super::support::{get_request, owner, request};
use crate::workflow::WorkflowCoordinatorError;
use crate::workflow::scheduler::{WorkflowCleanupStatus, WorkflowStopStatus};

#[tokio::test]
async fn bound_append_failure_contains_orphan_and_poisons_owner() {
    let run_id = WorkflowRunId::new("wrun_bind_failure");
    let journal = Arc::new(TestJournal::new());
    let (spawner, control) = test_spawner();
    let (handle, task, _, _) = coordinator(
        [100, 101, 102, 103, 104, 105],
        [run_id.clone()],
        [WorkflowAttemptId::new("watt_bind_failure")],
        journal.clone(),
        spawner,
    );
    let owner = owner("session-bind-failure", "root");
    let mut start = request("wreq_bind_failure");
    start.spec.nodes.remove(0);
    start.spec.nodes[0].dependencies.clear();
    handle.start(owner.clone(), start).await.unwrap();
    handle
        .schedule(owner.clone(), run_id.clone())
        .await
        .unwrap();

    let SpawnerEffect::Prepare { response, .. } = control.next().await else {
        panic!("expected prepare effect")
    };
    journal.push_append_error(WorkflowCoordinatorError::JournalUnavailable);
    let (worker, outcome) = prepared_worker("orphan-worker", 17);
    assert!(response.send(Ok(worker)).is_ok());

    let SpawnerEffect::Shutdown {
        execution,
        response,
        ..
    } = control.next().await
    else {
        panic!("expected orphan containment")
    };
    assert_eq!(execution.address.agent, "orphan-worker");
    assert_eq!(execution.connection_generation, 17);
    assert!(
        response
            .send(crate::workflow::scheduler::WorkflowCleanupStatus::Confirmed)
            .is_ok()
    );
    assert!(outcome.is_closed());
    control.assert_idle().await;

    assert_eq!(journal.events().len(), 2);
    assert_eq!(
        handle.tick(100_000).await,
        Err(WorkflowCoordinatorError::OwnerPoisoned),
        "a poisoned owner must reject deadline transitions"
    );
    assert_eq!(
        journal.events().len(),
        2,
        "a poisoned-owner tick must not append from stale state"
    );
    assert_eq!(
        handle
            .get(owner, get_request("wreq_bind_failure_get", run_id),)
            .await,
        Err(WorkflowCoordinatorError::OwnerPoisoned)
    );
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn terminal_append_failure_keeps_the_run_uncommitted_and_recontains_the_exact_lease() {
    let run_id = WorkflowRunId::new("wrun_terminal_failure");
    let journal = Arc::new(TestJournal::new());
    let (spawner, control) = test_spawner();
    let (handle, task, _, _) = coordinator(
        100..120,
        [run_id.clone()],
        [WorkflowAttemptId::new("watt_terminal_failure")],
        journal.clone(),
        spawner,
    );
    let owner = owner("session-terminal-failure", "root");
    let mut start = request("wreq_terminal_failure");
    start.spec.nodes.remove(0);
    start.spec.nodes[0].dependencies.clear();
    handle.start(owner.clone(), start).await.unwrap();
    handle
        .schedule(owner.clone(), run_id.clone())
        .await
        .unwrap();

    let SpawnerEffect::Prepare { response, .. } = control.next().await else {
        panic!("expected prepare effect")
    };
    let (worker, outcome) = prepared_worker("terminal-worker", 43);
    assert!(response.send(Ok(worker)).is_ok());
    let SpawnerEffect::Activate {
        execution,
        response,
    } = control.next().await
    else {
        panic!("expected activation effect")
    };
    assert!(response.send(Ok(())).is_ok());
    journal.wait_for_event_batches(4).await;

    handle
        .cancel(
            owner.clone(),
            WorkflowCancelRequest {
                request_id: WorkflowRequestId::new("wreq_terminal_failure_cancel"),
                run_id: run_id.clone(),
                reason: Some("stop before terminal append".into()),
            },
        )
        .await
        .unwrap();
    let SpawnerEffect::Interrupt {
        execution: interrupted,
        response,
    } = control.next().await
    else {
        panic!("expected interrupt effect")
    };
    assert_eq!(interrupted, execution);
    assert!(response.send(WorkflowStopStatus::Requested).is_ok());

    handle.tick(10_000).await.unwrap();
    let SpawnerEffect::Shutdown {
        execution: stopping,
        response,
        ..
    } = control.next().await
    else {
        panic!("expected shutdown effect")
    };
    assert_eq!(stopping, execution);
    journal.push_append_error(WorkflowCoordinatorError::JournalUnavailable);
    assert!(response.send(WorkflowCleanupStatus::Confirmed).is_ok());

    let SpawnerEffect::Shutdown {
        execution: contained,
        response,
        ..
    } = control.next().await
    else {
        panic!("expected exact lease containment after terminal append failure")
    };
    assert_eq!(contained, execution);
    assert!(response.send(WorkflowCleanupStatus::Confirmed).is_ok());
    assert!(outcome.is_closed());
    control.assert_idle().await;

    assert_eq!(
        handle
            .get(owner, get_request("wreq_terminal_failure_get", run_id),)
            .await,
        Err(WorkflowCoordinatorError::OwnerPoisoned)
    );
    assert_eq!(
        journal.events().len(),
        5,
        "terminal event must not be committed"
    );
    drop(handle);
    task.await.unwrap();
}
