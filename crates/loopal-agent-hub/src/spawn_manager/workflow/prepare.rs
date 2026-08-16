use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use loopal_protocol::{AgentCompletion, WorkflowAttemptFailure, WorkflowFailureClass};
use tokio::sync::oneshot;

use super::{PreparationOwner, ProductionWorkflowSpawner};
use crate::workflow::scheduler::{
    WorkflowPreparedWorker, WorkflowSpawnFailure, WorkflowSpawnRequest,
};

mod cleanup;
mod custody;
mod publication;
pub(super) mod registration;

use custody::PreparedRegistrationGuard;
use registration::PreparationRegistration;

pub(super) async fn run(
    spawner: &ProductionWorkflowSpawner,
    request: WorkflowSpawnRequest,
) -> Result<WorkflowPreparedWorker, WorkflowSpawnFailure> {
    run_with_fork(spawner, request, |hub, spawn, admission| async move {
        let process =
            super::super::spawn::fork_process_if(&hub, &spawn, move || admission.claim_fork())
                .await?;
        Ok((spawn, process))
    })
    .await
}

#[cfg(test)]
pub(in crate::spawn_manager::workflow) async fn run_with_process_for_test<P>(
    spawner: &ProductionWorkflowSpawner,
    request: WorkflowSpawnRequest,
    process: P,
) -> Result<WorkflowPreparedWorker, WorkflowSpawnFailure>
where
    P: super::super::spawn::SpawnProcess,
{
    run_with_fork(spawner, request, |_, spawn, admission| async move {
        admission.claim_fork()?;
        Ok((spawn, process))
    })
    .await
}

async fn run_with_fork<P, F, Fut>(
    spawner: &ProductionWorkflowSpawner,
    request: WorkflowSpawnRequest,
    fork: F,
) -> Result<WorkflowPreparedWorker, WorkflowSpawnFailure>
where
    P: super::super::spawn::SpawnProcess,
    F: FnOnce(
        Arc<tokio::sync::Mutex<crate::hub::Hub>>,
        crate::spawn_manager::PreparedSpawn,
        Arc<PreparationOwner>,
    ) -> Fut,
    Fut: Future<Output = Result<(crate::spawn_manager::PreparedSpawn, P), String>>,
{
    let attempt = request.causation.attempt_id.clone();
    let preparation = Arc::new(PreparationOwner::new(request.causation.clone()));
    {
        let mut owners = spawner.attempts.lock().await;
        if super::pre_abort::consume(&mut owners, &request.causation) {
            drop(owners);
            spawner.changed.notify_waiters();
            return Err(spawn_failure("workflow preparation was cancelled"));
        }
        if owners.preparing.contains_key(&attempt) || owners.by_attempt.contains_key(&attempt) {
            return Err(spawn_failure("duplicate or cancelled workflow attempt"));
        }
        owners
            .preparing
            .insert(attempt.clone(), preparation.clone());
    }
    let mut registration = PreparationRegistration::new(spawner, attempt.clone(), &preparation);
    let result = prepare_inner(spawner, request, preparation.clone(), fork).await;
    if result.is_err() {
        registration.remove().await;
    }
    registration.disarm();
    result
}

async fn prepare_inner<P, F, Fut>(
    spawner: &ProductionWorkflowSpawner,
    request: WorkflowSpawnRequest,
    preparation: Arc<PreparationOwner>,
    fork: F,
) -> Result<WorkflowPreparedWorker, WorkflowSpawnFailure>
where
    P: super::super::spawn::SpawnProcess,
    F: FnOnce(
        Arc<tokio::sync::Mutex<crate::hub::Hub>>,
        crate::spawn_manager::PreparedSpawn,
        Arc<PreparationOwner>,
    ) -> Fut,
    Fut: Future<Output = Result<(crate::spawn_manager::PreparedSpawn, P), String>>,
{
    let spawn = super::spawn_spec::build(spawner, &request)
        .await
        .map_err(spawn_failure)?;
    super::lifecycle_audit::append(
        spawner,
        &request.owner,
        &request.causation,
        None,
        super::lifecycle_audit::WorkflowAuditPhase::Prepare,
    )
    .await
    .map_err(spawn_failure)?;
    let (spawn, process) = fork(spawner.hub.clone(), spawn, preparation.clone())
        .await
        .map_err(spawn_failure)?;
    let prepared =
        super::super::spawn::prepare_and_register_process(spawner.hub.clone(), spawn, process)
            .await
            .map_err(spawn_failure)?;
    let execution = prepared.registered.execution.clone();
    let mut registration_guard = PreparedRegistrationGuard::new(spawner, execution.clone());
    let (outcome_tx, outcome) = oneshot::channel();
    let publication::Publication {
        cancelled,
        published,
        process,
        control,
    } = publication::run(spawner, &request, &preparation, &execution, prepared).await;
    registration_guard.disarm();
    if cancelled {
        if published {
            let _ = super::control::shutdown(spawner, &execution, Duration::from_secs(5)).await;
        } else if let Some(process) = process {
            cleanup::terminate_unowned(
                spawner,
                &request.owner,
                &request.causation,
                control,
                execution,
                process,
            )
            .await;
        }
        spawner.changed.notify_waiters();
        return Err(spawn_failure("workflow preparation was cancelled"));
    }
    spawner.changed.notify_waiters();
    super::monitor::spawn(
        spawner,
        execution.clone(),
        control,
        outcome_tx,
        request.output_contract,
    );
    Ok(WorkflowPreparedWorker { execution, outcome })
}

fn spawn_failure(reason: impl Into<String>) -> WorkflowSpawnFailure {
    let reason = reason.into();
    WorkflowSpawnFailure {
        completion: AgentCompletion::new("workflow_spawn_failed", Some(reason.clone())),
        failure: WorkflowAttemptFailure {
            class: WorkflowFailureClass::TransientBeforeExecution,
            reason,
        },
    }
}
