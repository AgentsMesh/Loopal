use std::sync::Arc;

use loopal_protocol::{
    AgentCompletion, WorkflowAttemptId, WorkflowEventPayload, WorkflowFailureClass, WorkflowRunId,
    WorkflowRunState,
};

use super::journal_support::TestJournal;
use super::scheduler_support::{SpawnerEffect, coordinator, prepared_worker, test_spawner};
use super::support::{get_request, owner, request};
use crate::workflow::scheduler::{WorkflowStopStatus, WorkflowWorkerOutcome};

#[path = "scheduler_deadline_pending.rs"]
mod pending;

#[tokio::test]
async fn run_deadline_stops_running_attempt_and_escalates_exact_lease() {
    let run_id = WorkflowRunId::new("wrun_deadline_active");
    let journal = Arc::new(TestJournal::new());
    let (spawner, control) = test_spawner();
    let (handle, task, _, _) = coordinator(
        [100, 101, 102, 103, 104, 105, 106, 107],
        [run_id.clone()],
        [WorkflowAttemptId::new("watt_deadline_active")],
        journal.clone(),
        spawner,
    );
    let owner = owner("session", "root");
    handle
        .start(owner.clone(), single_node_request("wreq_deadline_active"))
        .await
        .unwrap();
    handle
        .schedule(owner.clone(), run_id.clone())
        .await
        .unwrap();
    let SpawnerEffect::Prepare { response, .. } = control.next().await else {
        panic!("expected prepare effect")
    };
    let (worker, outcome) = prepared_worker("worker", 9);
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

    handle.tick(60_100).await.unwrap();
    let SpawnerEffect::Interrupt {
        execution: interrupted,
        response,
    } = control.next().await
    else {
        panic!("expected interrupt effect")
    };
    assert_eq!(interrupted, execution);
    assert!(matches!(
        last_payload(&journal),
        WorkflowEventPayload::AttemptStopRequested { .. }
    ));
    assert!(response.send(WorkflowStopStatus::Requested).is_ok());

    handle.tick(61_100).await.unwrap();
    let SpawnerEffect::Shutdown {
        execution: shutdown,
        response,
        ..
    } = control.next().await
    else {
        panic!("expected shutdown effect")
    };
    assert_eq!(shutdown, execution);
    assert!(
        response
            .send(crate::workflow::scheduler::WorkflowCleanupStatus::Confirmed)
            .is_ok()
    );
    journal.wait_for_event_batches(6).await;

    let run = get_run(&handle, owner, run_id, "wreq_deadline_active_get").await;
    assert!(
        outcome
            .send(WorkflowWorkerOutcome::Succeeded {
                completion: AgentCompletion::goal(Some("late".into())),
                output: None,
            })
            .is_err()
    );
    assert_eq!(run.state, WorkflowRunState::Failed);
    assert_eq!(run.failure.unwrap().class, WorkflowFailureClass::Permanent);
    assert!(matches!(
        last_payload(&journal),
        WorkflowEventPayload::AttemptFailed { .. }
    ));
    drop(handle);
    task.await.unwrap();
}

fn single_node_request(id: &str) -> loopal_protocol::WorkflowStartRequest {
    let mut value = request(id);
    value.spec.nodes.remove(0);
    value.spec.nodes[0].dependencies.clear();
    value
}

async fn get_run(
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

fn last_payload(journal: &TestJournal) -> WorkflowEventPayload {
    journal
        .events()
        .into_iter()
        .flat_map(|(_, _, events)| events)
        .last()
        .unwrap()
        .payload
}
