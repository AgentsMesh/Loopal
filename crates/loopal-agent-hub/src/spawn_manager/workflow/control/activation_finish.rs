use loopal_protocol::{WorkflowAttemptFailure, WorkflowFailureClass};

use super::super::{AttemptPhase, ProductionWorkflowSpawner};
use crate::types::AgentExecutionRef;
use crate::workflow::scheduler::WorkflowActivationFailure;

pub(super) async fn finish(
    spawner: &ProductionWorkflowSpawner,
    execution: &AgentExecutionRef,
) -> Result<(), WorkflowActivationFailure> {
    finish_inner(spawner, execution, || {}).await
}

#[cfg(test)]
pub(in crate::spawn_manager::workflow) async fn finish_activation_for_test(
    spawner: &ProductionWorkflowSpawner,
    execution: &AgentExecutionRef,
    reached_hub_wait: std::sync::Arc<tokio::sync::Notify>,
) -> Result<(), WorkflowActivationFailure> {
    finish_inner(spawner, execution, || reached_hub_wait.notify_one()).await
}

async fn finish_inner(
    spawner: &ProductionWorkflowSpawner,
    execution: &AgentExecutionRef,
    reached_hub_wait: impl FnOnce(),
) -> Result<(), WorkflowActivationFailure> {
    {
        let mut owners = spawner.attempts.lock().await;
        ensure_activating(&mut owners, execution)?;
    }

    reached_hub_wait();
    let lease_active = {
        let hub = spawner.hub.lock().await;
        hub.registry.owns_active_lease(execution)
    };
    if !lease_active {
        return Err(uncertain("workflow lease changed during agent/start"));
    }

    let mut owners = spawner.attempts.lock().await;
    ensure_activating(&mut owners, execution)?;
    super::exact_mut(&mut owners, execution)
        .expect("validated exact owner")
        .phase = AttemptPhase::Running;
    drop(owners);

    let mut hub = spawner.hub.lock().await;
    if !hub.registry.owns_active_lease(execution) {
        drop(hub);
        mark_stopping_if_running(spawner, execution).await;
        return Err(uncertain("workflow lease changed during agent/start"));
    }
    hub.registry
        .set_lifecycle(&execution.address.agent, crate::AgentLifecycle::Running);
    Ok(())
}

fn ensure_activating(
    owners: &mut super::super::AttemptOwners,
    execution: &AgentExecutionRef,
) -> Result<(), WorkflowActivationFailure> {
    match super::exact_mut(owners, execution) {
        None => Err(uncertain("workflow owner disappeared after agent/start")),
        Some(attempt) if attempt.phase != AttemptPhase::Activating => {
            Err(uncertain("workflow owner stopped during agent/start"))
        }
        Some(_) => Ok(()),
    }
}

async fn mark_stopping_if_running(
    spawner: &ProductionWorkflowSpawner,
    execution: &AgentExecutionRef,
) {
    let mut owners = spawner.attempts.lock().await;
    if let Some(attempt) = super::exact_mut(&mut owners, execution)
        && attempt.phase == AttemptPhase::Running
    {
        attempt.phase = AttemptPhase::Stopping;
    }
}

fn uncertain(reason: &str) -> WorkflowActivationFailure {
    WorkflowActivationFailure::Uncertain(WorkflowAttemptFailure {
        class: WorkflowFailureClass::AmbiguousExecution,
        reason: reason.into(),
    })
}
