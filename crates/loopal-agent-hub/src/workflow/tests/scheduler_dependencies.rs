use std::sync::Arc;

use loopal_protocol::{
    AgentCompletion, WorkflowAttemptId, WorkflowAttemptState, WorkflowFailureClass, WorkflowRunId,
    WorkflowRunState,
};

use super::journal_support::TestJournal;
use super::scheduler_support::{SpawnerEffect, coordinator, prepared_worker, test_spawner};
use super::support::{get_request, owner, request};
use crate::workflow::scheduler::WorkflowWorkerOutcome;

#[tokio::test]
async fn ready_node_without_authoritative_dependency_result_fails_durably_without_spawn() {
    let run_id = WorkflowRunId::new("wrun_missing_dependency_result");
    let journal = Arc::new(TestJournal::new());
    let (spawner, control) = test_spawner();
    let (handle, task, _, _) = coordinator(
        100..140,
        [run_id.clone()],
        [
            WorkflowAttemptId::new("watt_source_without_result"),
            WorkflowAttemptId::new("watt_blocked_output"),
        ],
        journal.clone(),
        spawner,
    );
    let owner = owner("session-missing-result", "root");

    handle
        .start(owner.clone(), request("wreq_missing_dependency_result"))
        .await
        .unwrap();
    handle
        .schedule(owner.clone(), run_id.clone())
        .await
        .unwrap();
    let SpawnerEffect::Prepare {
        request: source_request,
        response: prepare,
    } = control.next().await
    else {
        panic!("expected source preparation")
    };
    assert!(source_request.dependency_results.is_empty());
    let (worker, outcome) = prepared_worker("source-worker", 1);
    assert!(prepare.send(Ok(worker)).is_ok());
    let SpawnerEffect::Activate { response, .. } = control.next().await else {
        panic!("expected source activation")
    };
    assert!(response.send(Ok(())).is_ok());
    journal.wait_for_event_batches(4).await;

    outcome
        .send(WorkflowWorkerOutcome::Succeeded {
            completion: AgentCompletion::goal(None),
            output: None,
        })
        .unwrap();
    journal.wait_for_event_batches(7).await;
    control.assert_idle().await;

    let run = handle
        .get(
            owner,
            get_request("wreq_missing_dependency_result_get", run_id),
        )
        .await
        .unwrap()
        .run
        .unwrap();
    assert_eq!(run.state, WorkflowRunState::Failed);
    let blocked = run
        .attempts
        .iter()
        .find(|attempt| attempt.id.as_str() == "watt_blocked_output")
        .expect("blocked output attempt must be terminalized durably");
    assert_eq!(blocked.state, WorkflowAttemptState::Failed);
    assert_eq!(
        blocked.completion.as_ref().unwrap().reason,
        "workflow_dependency_result_unavailable"
    );
    assert_eq!(
        blocked.failure.as_ref().unwrap().class,
        WorkflowFailureClass::Permanent
    );

    drop(handle);
    task.await.unwrap();
}
