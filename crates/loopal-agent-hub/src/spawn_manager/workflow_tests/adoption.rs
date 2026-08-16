use loopal_protocol::{AgentCompletion, QualifiedAddress};

use super::support::harness;
use crate::types::AgentExecutionRef;
use crate::workflow::WorkflowOwner;
use crate::workflow::scheduler::{
    WorkflowRecoveryAdoptionError, WorkflowRecoveryAdoptionRequest, WorkflowSpawner,
    WorkflowWorkerOutcome,
};

#[tokio::test]
async fn exact_running_process_custody_rebuilds_completion_monitor_once() {
    let fixture = harness().await;
    mark_running(&fixture.spawner, &fixture.causation.attempt_id).await;
    let request = WorkflowRecoveryAdoptionRequest {
        owner: WorkflowOwner::new("session", QualifiedAddress::local("root")),
        causation: fixture.causation.clone(),
        execution: fixture.execution.clone(),
        output_contract: None,
    };
    let worker = fixture
        .spawner
        .adopt_recovered(request.clone())
        .await
        .unwrap();
    assert!(matches!(
        fixture.spawner.adopt_recovered(request).await,
        Err(WorkflowRecoveryAdoptionError::MissingCustody)
    ));

    let mut delivery = fixture
        .spawner
        .hub
        .lock()
        .await
        .registry
        .emit_agent_completion("workflow-watt_owner", AgentCompletion::goal(None));
    delivery.deliver_events().await.unwrap();
    let outcome = tokio::time::timeout(std::time::Duration::from_secs(1), worker.outcome)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(outcome, WorkflowWorkerOutcome::Succeeded { .. }));
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            if !fixture
                .spawner
                .attempts
                .lock()
                .await
                .by_attempt
                .contains_key(&fixture.causation.attempt_id)
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let owners = fixture.spawner.attempts.lock().await;
    assert!(
        !owners
            .by_attempt
            .contains_key(&fixture.causation.attempt_id)
    );
    assert!(!owners.by_execution.contains_key(&fixture.execution));
}

#[tokio::test]
async fn adoption_rejects_non_exact_or_incomplete_process_custody() {
    let fixture = harness().await;
    let request = WorkflowRecoveryAdoptionRequest {
        owner: WorkflowOwner::new("session", QualifiedAddress::local("root")),
        causation: fixture.causation.clone(),
        execution: fixture.execution.clone(),
        output_contract: None,
    };
    assert!(matches!(
        fixture.spawner.adopt_recovered(request.clone()).await,
        Err(WorkflowRecoveryAdoptionError::InvalidPhase)
    ));

    mark_running(&fixture.spawner, &fixture.causation.attempt_id).await;
    let mut stale = request.clone();
    stale.execution = AgentExecutionRef::local(
        fixture.execution.address.agent.clone(),
        fixture.execution.connection_generation + 1,
    );
    assert!(matches!(
        fixture.spawner.adopt_recovered(stale).await,
        Err(WorkflowRecoveryAdoptionError::StaleExecution)
    ));
    let mut conflict = request.clone();
    conflict.owner = WorkflowOwner::new("other-session", QualifiedAddress::local("root"));
    assert!(matches!(
        fixture.spawner.adopt_recovered(conflict).await,
        Err(WorkflowRecoveryAdoptionError::ConflictingOwner)
    ));

    let process = fixture
        .spawner
        .attempts
        .lock()
        .await
        .by_attempt
        .get_mut(&fixture.causation.attempt_id)
        .unwrap()
        .process
        .take()
        .unwrap();
    assert!(matches!(
        fixture.spawner.adopt_recovered(request).await,
        Err(WorkflowRecoveryAdoptionError::MissingCustody)
    ));
    let _ = process.shutdown().await;
}

async fn mark_running(
    spawner: &super::super::ProductionWorkflowSpawner,
    attempt_id: &loopal_protocol::WorkflowAttemptId,
) {
    spawner
        .attempts
        .lock()
        .await
        .by_attempt
        .get_mut(attempt_id)
        .unwrap()
        .phase = super::super::AttemptPhase::Running;
}
