use std::sync::Arc;

use loopal_protocol::{
    AgentCompletion, WorkflowAttemptCapability, WorkflowAttemptId, WorkflowAttemptState,
    WorkflowNodeId, WorkflowOutput, WorkflowRunId, WorkflowWorkerHandshakeDisposition,
};

use super::journal_support::TestJournal;
use super::scheduler_reconnect_support::begin_handshake;
use super::scheduler_support::{SpawnerEffect, coordinator, prepared_worker, test_spawner};
use super::support::{owner, request};
use crate::types::AgentExecutionRef;
use crate::workflow::WorkflowCoordinatorError;
use crate::workflow::scheduler::WorkflowWorkerOutcome;

#[tokio::test]
async fn fresh_worker_handshake_validates_the_exact_live_execution_lease() {
    let run_id = WorkflowRunId::new("wrun_fresh_handshake");
    let attempt_id = WorkflowAttemptId::new("watt_fresh_handshake");
    let journal = Arc::new(TestJournal::new());
    let (spawner, control) = test_spawner();
    let (handle, task, _, _) = coordinator(
        100..120,
        [run_id.clone()],
        [attempt_id],
        journal.clone(),
        spawner,
    );
    let owner = owner("session-fresh-handshake", "root");
    let mut start = request("wreq_fresh_handshake");
    start.spec.nodes.remove(0);
    start.spec.nodes[0].dependencies.clear();
    handle.start(owner.clone(), start).await.unwrap();
    handle.schedule(owner.clone(), run_id).await.unwrap();

    let SpawnerEffect::Prepare {
        request: spawn,
        response: prepare,
    } = control.next().await
    else {
        panic!("expected workflow prepare effect")
    };
    let causation = spawn.causation;
    let capability = spawn.attempt_capability;
    let execution = AgentExecutionRef::local("worker", 7);
    let (worker, outcome) = prepared_worker("worker", 7);
    assert!(prepare.send(Ok(worker)).is_ok());
    let SpawnerEffect::Activate {
        execution: activating,
        response: activate,
    } = control.next().await
    else {
        panic!("expected workflow activation effect")
    };
    assert_eq!(activating, execution);

    let fresh = begin_handshake(
        handle.clone(),
        owner.clone(),
        causation.clone(),
        capability.clone(),
        execution.clone(),
    )
    .await
    .unwrap();
    assert_eq!(fresh.disposition, WorkflowWorkerHandshakeDisposition::Fresh);
    assert_eq!(fresh.attempt_state, WorkflowAttemptState::Dispatching);

    let mut invalid_causation = causation.clone();
    invalid_causation.node_id = WorkflowNodeId::new("");
    assert_eq!(
        begin_handshake(
            handle.clone(),
            owner.clone(),
            invalid_causation,
            capability.clone(),
            execution.clone(),
        )
        .await,
        Err(WorkflowCoordinatorError::InvalidExecutionLease)
    );
    assert_eq!(
        begin_handshake(
            handle.clone(),
            owner.clone(),
            causation.clone(),
            capability.clone(),
            AgentExecutionRef::local("worker", 0),
        )
        .await,
        Err(WorkflowCoordinatorError::InvalidExecutionLease)
    );

    let mut unknown_attempt = causation.clone();
    unknown_attempt.attempt_id = WorkflowAttemptId::new("watt_unknown_handshake");
    assert_eq!(
        begin_handshake(
            handle.clone(),
            owner.clone(),
            unknown_attempt,
            capability.clone(),
            execution.clone(),
        )
        .await,
        Err(WorkflowCoordinatorError::RecoveryInvalid)
    );

    let mut wrong_node = causation.clone();
    wrong_node.node_id = WorkflowNodeId::new("other");
    assert_eq!(
        begin_handshake(
            handle.clone(),
            owner.clone(),
            wrong_node,
            capability.clone(),
            execution.clone(),
        )
        .await,
        Err(WorkflowCoordinatorError::InvalidExecutionLease)
    );
    assert_eq!(
        begin_handshake(
            handle.clone(),
            owner.clone(),
            causation.clone(),
            WorkflowAttemptCapability::parse("22".repeat(32)).unwrap(),
            execution.clone(),
        )
        .await,
        Err(WorkflowCoordinatorError::InvalidExecutionLease)
    );
    assert_eq!(
        begin_handshake(
            handle.clone(),
            owner.clone(),
            causation.clone(),
            capability.clone(),
            AgentExecutionRef::local("other-worker", 7),
        )
        .await,
        Err(WorkflowCoordinatorError::InvalidExecutionLease)
    );
    assert_eq!(
        begin_handshake(
            handle.clone(),
            owner.clone(),
            causation.clone(),
            capability.clone(),
            AgentExecutionRef::local("worker", 8),
        )
        .await,
        Err(WorkflowCoordinatorError::StaleExecutionLease)
    );

    assert!(activate.send(Ok(())).is_ok());
    journal.wait_for_event_batches(4).await;
    let running = begin_handshake(
        handle.clone(),
        owner.clone(),
        causation.clone(),
        capability.clone(),
        execution.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        running.disposition,
        WorkflowWorkerHandshakeDisposition::Fresh
    );
    assert_eq!(running.attempt_state, WorkflowAttemptState::Running);

    outcome
        .send(WorkflowWorkerOutcome::Succeeded {
            completion: AgentCompletion::goal(Some("done".into())),
            output: Some(WorkflowOutput::Text("done".into())),
        })
        .unwrap();
    journal.wait_for_event_batches(5).await;
    assert_eq!(
        begin_handshake(handle.clone(), owner, causation, capability, execution).await,
        Err(WorkflowCoordinatorError::InvalidExecutionLease)
    );

    control.assert_idle().await;
    handle.shutdown().await.unwrap();
    task.await.unwrap();
}
