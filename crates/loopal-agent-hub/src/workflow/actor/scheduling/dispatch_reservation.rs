use std::time::Duration;

use loopal_protocol::{
    AgentCompletion, WorkflowAttemptFailure, WorkflowEventPayload, WorkflowFailureClass,
    WorkflowNodeId, WorkflowPermissionCausation, WorkflowRunSnapshot,
};

use super::super::super::WorkflowCoordinator;
use super::super::commit;
use crate::workflow::command::WorkflowCommand;
use crate::workflow::scheduler::{
    AttemptKey, PendingAttempt, WorkflowDependencyResult, WorkflowSpawnFailure,
    WorkflowSpawnRequest, prepare_spawn_failure, resolve_dependency_results,
};
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

pub(super) async fn reserve(
    coordinator: &mut WorkflowCoordinator,
    owner: &WorkflowOwner,
    run: WorkflowRunSnapshot,
    node_id: WorkflowNodeId,
) -> Result<WorkflowRunSnapshot, WorkflowCoordinatorError> {
    let attempt_id = coordinator.ids.next_attempt_id();
    let attempt_capability = coordinator.ids.next_attempt_capability();
    if !attempt_id.is_valid() {
        return Err(WorkflowCoordinatorError::InvalidGeneratedAttemptId(
            attempt_id,
        ));
    }
    if run.attempts.iter().any(|attempt| attempt.id == attempt_id)
        || coordinator.active.contains_key(&attempt_id)
        || coordinator.pending.contains_key(&attempt_id)
    {
        return Err(WorkflowCoordinatorError::AttemptIdCollision(attempt_id));
    }
    let key = AttemptKey {
        run_id: run.id.clone(),
        node_id: node_id.clone(),
        attempt_id: attempt_id.clone(),
    };
    let now = coordinator.clock.now_unix_ms();
    let next = commit::payload(
        coordinator,
        owner,
        &run,
        WorkflowEventPayload::DispatchIntended {
            node_id: node_id.clone(),
            attempt_id,
            capability_digest: attempt_capability.digest(),
        },
        now,
    )
    .await?;
    let dependency_results =
        match resolve_dependency_results(&next, &key.node_id, &coordinator.redaction_seed) {
            Ok(results) => results,
            Err(failure) => {
                let payload =
                    prepare_spawn_failure(&next, &key, failure, &coordinator.redaction_seed)
                        .payload;
                let now = coordinator.clock.now_unix_ms();
                return commit::payload(coordinator, owner, &next, payload, now).await;
            }
        };
    coordinator.pending.insert(
        key.attempt_id.clone(),
        PendingAttempt {
            owner: owner.clone(),
            key: key.clone(),
            causation: WorkflowPermissionCausation {
                run_id: key.run_id.clone(),
                node_id: key.node_id.clone(),
                attempt_id: key.attempt_id.clone(),
            },
            deadline_unix_ms: now.saturating_add(next.spec.limits.attempt_timeout_ms),
            prepare_abort: None,
            abort_waiter: None,
            abort_requested: false,
            abort_status: None,
            delivery_finished: false,
            late_execution: None,
            late_shutdown_waiter: None,
            stop: None,
        },
    );
    let prepare = spawn_prepare(
        coordinator,
        owner.clone(),
        &next,
        key.clone(),
        dependency_results,
        attempt_capability,
    );
    if let Some(pending) = coordinator.pending.get_mut(&key.attempt_id) {
        pending.prepare_abort = Some(prepare);
    }
    Ok(next)
}

fn spawn_prepare(
    coordinator: &WorkflowCoordinator,
    owner: WorkflowOwner,
    run: &WorkflowRunSnapshot,
    key: AttemptKey,
    dependency_results: Vec<WorkflowDependencyResult>,
    attempt_capability: loopal_protocol::WorkflowAttemptCapability,
) -> tokio::task::JoinHandle<()> {
    let node = run
        .spec
        .nodes
        .iter()
        .find(|node| node.id == key.node_id)
        .expect("validated node exists");
    let worker_profile =
        crate::workflow::worker_profile::resolve_node_profile(&node.id, &node.worker_profile)
            .expect("workflow profiles are validated before dispatch");
    let request = WorkflowSpawnRequest {
        owner: owner.clone(),
        causation: WorkflowPermissionCausation {
            run_id: key.run_id.clone(),
            node_id: key.node_id.clone(),
            attempt_id: key.attempt_id.clone(),
        },
        run_goal: run.spec.run_goal.clone(),
        task: node.task.clone(),
        dependency_results,
        worker_profile,
        output_contract: (key.node_id == run.spec.output_node)
            .then(|| run.spec.output_contract.clone()),
        completion_result_limit: run.spec.limits.max_output_bytes,
        attempt_capability,
    };
    let spawner = coordinator.spawner.clone();
    let callbacks = coordinator.callbacks.clone();
    let timeout = Duration::from_millis(run.spec.limits.attempt_timeout_ms);
    tokio::spawn(async move {
        let prepared = match tokio::time::timeout(timeout, spawner.prepare(request)).await {
            Ok(result) => result,
            Err(_) => {
                let Some(callbacks) = callbacks.upgrade() else {
                    return;
                };
                let _ = callbacks
                    .send(WorkflowCommand::WorkerPreparationTimedOut {
                        owner,
                        key,
                        failure: preparation_timeout(),
                    })
                    .await;
                return;
            }
        };
        let delivery =
            crate::workflow::scheduler::WorkflowPreparedDelivery::new(prepared, spawner.clone());
        let Some(callbacks) = callbacks.upgrade() else {
            return;
        };
        if callbacks
            .send(WorkflowCommand::WorkerPrepared {
                owner: owner.clone(),
                key: key.clone(),
                prepared: delivery,
            })
            .await
            .is_ok()
        {
            let _ = callbacks
                .send(WorkflowCommand::PreparationDeliveryFinished { owner, key })
                .await;
        }
    })
}

fn preparation_timeout() -> WorkflowSpawnFailure {
    WorkflowSpawnFailure {
        completion: AgentCompletion::new("workflow_prepare_timeout", None),
        failure: WorkflowAttemptFailure {
            class: WorkflowFailureClass::TransientBeforeExecution,
            reason: "workflow worker preparation timed out".into(),
        },
    }
}
