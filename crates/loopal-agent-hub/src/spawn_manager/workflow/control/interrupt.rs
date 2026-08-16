use super::super::{AttemptPhase, ProductionWorkflowSpawner};
use crate::types::AgentExecutionRef;
use crate::workflow::scheduler::WorkflowStopStatus;

pub(in crate::spawn_manager::workflow) async fn interrupt(
    spawner: &ProductionWorkflowSpawner,
    execution: &AgentExecutionRef,
) -> WorkflowStopStatus {
    let operation = {
        let mut owners = spawner.attempts.lock().await;
        let Some(attempt) = super::exact_mut(&mut owners, execution) else {
            return WorkflowStopStatus::Stopped;
        };
        attempt.operation.clone()
    };
    let _operation = operation.lock().await;
    let (owner, causation) = {
        let mut owners = spawner.attempts.lock().await;
        let Some(attempt) = super::exact_mut(&mut owners, execution) else {
            return WorkflowStopStatus::Stopped;
        };
        attempt.phase = AttemptPhase::Stopping;
        (attempt.owner.clone(), attempt.causation.clone())
    };
    super::super::lifecycle_audit::append_before_cleanup(
        spawner,
        &owner,
        &causation,
        Some(execution),
        super::super::lifecycle_audit::WorkflowAuditPhase::Interrupt,
    )
    .await;
    let operation = spawner.hub.lock().await.registry.interrupt_exact(execution);
    let Some(operation) = operation else {
        return if !still_owned(spawner, execution).await {
            WorkflowStopStatus::Stopped
        } else {
            WorkflowStopStatus::Requested
        };
    };
    if operation.execute(&execution.address).await {
        WorkflowStopStatus::Requested
    } else {
        if still_owned(spawner, execution).await {
            WorkflowStopStatus::Requested
        } else {
            WorkflowStopStatus::Stopped
        }
    }
}

async fn still_owned(spawner: &ProductionWorkflowSpawner, execution: &AgentExecutionRef) -> bool {
    let mut owners = spawner.attempts.lock().await;
    super::exact_mut(&mut owners, execution).is_some()
}
