use std::sync::Arc;

use loopal_protocol::{
    AgentCompletion, WorkflowAttemptId, WorkflowEventPayload, WorkflowOutput, WorkflowRunId,
    WorkflowRunState,
};

use super::journal_support::TestJournal;
use super::scheduler_support::{SpawnerEffect, coordinator, prepared_worker, test_spawner};
use super::support::{get_request, owner, request};
use crate::workflow::scheduler::WorkflowWorkerOutcome;

#[tokio::test]
async fn lifecycle_journals_each_transition_before_worker_effects() {
    let run_id = WorkflowRunId::new("wrun_lifecycle");
    let attempt_id = WorkflowAttemptId::new("watt_lifecycle");
    let journal = Arc::new(TestJournal::new());
    let (spawner, control) = test_spawner();
    let (handle, task, _, ids) = coordinator(
        [100, 101, 102, 103, 104, 105, 106, 107],
        [run_id.clone()],
        [attempt_id.clone()],
        journal.clone(),
        spawner,
    );
    let owner = owner("session", "root");
    let mut start = request("wreq_lifecycle");
    start.spec.nodes.remove(0);
    start.spec.nodes[0].dependencies.clear();

    handle.start(owner.clone(), start).await.unwrap();
    handle
        .schedule(owner.clone(), run_id.clone())
        .await
        .unwrap();

    let SpawnerEffect::Prepare {
        request: spawn,
        response: prepare,
    } = control.next().await
    else {
        panic!("expected workflow prepare effect")
    };
    assert_eq!(
        event_types(&journal),
        vec!["run_started", "dispatch_intended"]
    );
    assert_eq!(spawn.owner, owner);
    assert_eq!(spawn.causation.run_id, run_id);
    assert_eq!(spawn.causation.node_id.as_str(), "output");
    assert_eq!(spawn.causation.attempt_id, attempt_id);
    assert_eq!(spawn.run_goal, "complete the workflow");
    assert_eq!(spawn.task, "complete output");
    assert_eq!(spawn.worker_profile.agent_type(), "default");

    let (worker, outcome) = prepared_worker("worker", 7);
    assert!(prepare.send(Ok(worker)).is_ok());
    let SpawnerEffect::Activate {
        execution,
        response: activate,
    } = control.next().await
    else {
        panic!("expected workflow activation effect")
    };
    assert_eq!(event_types(&journal).last(), Some(&"attempt_bound"));
    assert_eq!(execution.address.agent, "worker");
    assert_eq!(execution.connection_generation, 7);

    assert!(activate.send(Ok(())).is_ok());
    journal.wait_for_event_batches(4).await;
    assert_eq!(event_types(&journal).last(), Some(&"attempt_running"));
    outcome
        .send(WorkflowWorkerOutcome::Succeeded {
            completion: AgentCompletion::goal(Some("done".into())),
            output: Some(WorkflowOutput::Text("answer".into())),
        })
        .unwrap();
    journal.wait_for_event_batches(5).await;

    assert_eq!(
        event_types(&journal),
        vec![
            "run_started",
            "dispatch_intended",
            "attempt_bound",
            "attempt_running",
            "attempt_succeeded",
        ]
    );
    let run = handle
        .get(
            owner,
            get_request("wreq_lifecycle_get", WorkflowRunId::new("wrun_lifecycle")),
        )
        .await
        .unwrap()
        .run
        .unwrap();
    assert_eq!(run.state, WorkflowRunState::Succeeded);
    assert_eq!(run.result, Some(WorkflowOutput::Text("answer".into())));
    assert_eq!(ids.attempt_calls(), 1);

    drop(handle);
    task.await.unwrap();
}

fn event_types(journal: &TestJournal) -> Vec<&'static str> {
    journal
        .events()
        .into_iter()
        .flat_map(|(_, _, events)| events)
        .map(|event| match event.payload {
            WorkflowEventPayload::RunStarted => "run_started",
            WorkflowEventPayload::DispatchIntended { .. } => "dispatch_intended",
            WorkflowEventPayload::AttemptBound { .. } => "attempt_bound",
            WorkflowEventPayload::AttemptRunning { .. } => "attempt_running",
            WorkflowEventPayload::AttemptSucceeded { .. } => "attempt_succeeded",
            _ => "other",
        })
        .collect()
}
