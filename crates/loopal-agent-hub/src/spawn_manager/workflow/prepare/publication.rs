use std::sync::Arc;

use super::super::{AttemptOwner, AttemptPhase, PreparationOwner, ProductionWorkflowSpawner};
use super::registration;
use crate::spawn_manager::spawn::{
    PreparedAgentProcess, PreparedControl, SpawnProcess, WorkflowProcessOwner,
};
use crate::types::AgentExecutionRef;
use crate::workflow::scheduler::WorkflowSpawnRequest;

pub(super) struct Publication {
    pub(super) cancelled: bool,
    pub(super) published: bool,
    pub(super) process: Option<WorkflowProcessOwner>,
    pub(super) control: Arc<PreparedControl>,
}

pub(super) async fn run<P: SpawnProcess>(
    spawner: &ProductionWorkflowSpawner,
    request: &WorkflowSpawnRequest,
    preparation: &Arc<PreparationOwner>,
    execution: &AgentExecutionRef,
    prepared: PreparedAgentProcess<P>,
) -> Publication {
    let mut owners = spawner.attempts.lock().await;
    if owners.by_execution.contains_key(execution) {
        let (process, control) = prepared.into_workflow_parts();
        return Publication {
            cancelled: true,
            published: false,
            process: Some(process),
            control: Arc::new(control),
        };
    }

    let cancelled = preparation.is_cancelled();
    let (process, control) = prepared.into_workflow_parts();
    let control = Arc::new(control);
    owners
        .by_execution
        .insert(execution.clone(), request.causation.attempt_id.clone());
    owners.by_attempt.insert(
        request.causation.attempt_id.clone(),
        AttemptOwner {
            owner: request.owner.clone(),
            causation: request.causation.clone(),
            execution: execution.clone(),
            control: control.clone(),
            process: Some(process),
            process_shutdown: None,
            cleanup_registered: cancelled,
            operation: Arc::new(tokio::sync::Mutex::new(())),
            phase: if cancelled {
                AttemptPhase::Stopping
            } else {
                AttemptPhase::Prepared
            },
        },
    );
    registration::remove_preparation(&mut owners, &request.causation.attempt_id, preparation);
    Publication {
        cancelled,
        published: true,
        process: None,
        control,
    }
}
