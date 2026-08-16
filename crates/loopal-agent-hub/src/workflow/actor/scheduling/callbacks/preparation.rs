use loopal_protocol::{WorkflowAttemptState, WorkflowFailureClass, WorkflowRunState};

use super::super::super::WorkflowCoordinator;
use super::super::{commit, dispatch};
use crate::workflow::scheduler::{
    AttemptKey, StopDisposition, WorkflowPreparedDelivery, prepare_spawn_failure,
};
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

pub(in crate::workflow::actor) fn preparation_timed_out(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    failure: crate::workflow::scheduler::WorkflowSpawnFailure,
) {
    let Some(pending) = coordinator.pending.get_mut(&key.attempt_id) else {
        return;
    };
    if pending.owner != owner || pending.key != key {
        return;
    }

    // The wall-clock timer owns no external cleanup. It only closes local
    // preparation delivery and lets the actor's one-shot abort path arbitrate
    // with cancellation and deadline ticks for this exact causation.
    pending.delivery_finished = true;
    if pending.stop.is_none() {
        pending.stop = Some(StopDisposition::Failed(failure));
    }
    super::super::stop::request_pending_attempt_abort(coordinator, &owner, &key);
}

pub(in crate::workflow::actor) async fn prepared(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    prepared: WorkflowPreparedDelivery,
) -> Result<(), WorkflowCoordinatorError> {
    let (abort_requested, late_execution) = {
        let Some(pending) = coordinator.pending.get(&key.attempt_id) else {
            return Ok(());
        };
        if pending.owner != owner || pending.key != key {
            return Ok(());
        }
        (pending.abort_requested, pending.late_execution.is_some())
    };
    if coordinator.state.is_poisoned(&owner) {
        return Ok(());
    }
    if let Some(pending) = coordinator.pending.get_mut(&key.attempt_id) {
        pending.delivery_finished = true;
    }
    // A late success is already being contained for this tombstone. Dropping
    // the duplicate delivery contains it without replacing the first exact
    // execution identity or its shutdown acknowledgement.
    if late_execution {
        return Ok(());
    }
    let run = coordinator.scheduler_snapshot(&owner, &key.run_id)?;
    if abort_requested {
        let mut pending = coordinator
            .pending
            .remove(&key.attempt_id)
            .expect("validated pending attempt exists");
        match prepared.into_result() {
            Err(failure) => {
                // Any failed preparation can race with cancellation. Keep the
                // tombstone and let the causation-bound abort acknowledgement
                // decide whether cleanup is confirmed or ambiguous. Preserve
                // an ambiguous failure so a later Confirmed acknowledgement
                // cannot accidentally turn an uncertain timeout into a
                // cancellation.
                if failure.failure.class == WorkflowFailureClass::AmbiguousExecution {
                    pending.stop = Some(StopDisposition::Failed(failure));
                }
                coordinator.pending.insert(key.attempt_id.clone(), pending);
                return Ok(());
            }
            Ok(worker) => {
                pending.prepare_abort.take();
                pending.late_execution = Some(worker.execution.clone());
                coordinator.pending.insert(key.attempt_id.clone(), pending);
                super::contain_late_preparation(coordinator, owner, key, worker);
                return Ok(());
            }
        }
    }
    let pending = coordinator
        .pending
        .remove(&key.attempt_id)
        .expect("validated pending attempt exists");
    let prepared = prepared.into_result();
    if run.state == WorkflowRunState::Cancelling || pending.stop.is_some() {
        return super::super::stop::finish_preparation_stop(
            coordinator,
            owner,
            key,
            prepared,
            run,
            pending,
        )
        .await;
    }
    let attempt = run
        .attempts
        .iter()
        .find(|attempt| attempt.id == key.attempt_id);
    if run.state != WorkflowRunState::Running
        || attempt.is_none_or(|attempt| {
            attempt.node_id != key.node_id || attempt.state != WorkflowAttemptState::Dispatching
        })
    {
        if let Ok(worker) = prepared {
            coordinator.contain_execution(worker.execution);
        }
        return Ok(());
    }
    match prepared {
        Err(failure) => {
            let payload =
                prepare_spawn_failure(&run, &key, failure, &coordinator.redaction_seed).payload;
            commit::payload(
                coordinator,
                &owner,
                &run,
                payload,
                coordinator.clock.now_unix_ms(),
            )
            .await?;
            dispatch::admit(coordinator, owner, key.run_id).await
        }
        Ok(worker) => {
            super::preparation_activation::bind_and_activate(
                coordinator,
                owner,
                key,
                worker,
                run,
                pending.deadline_unix_ms,
            )
            .await
        }
    }
}

pub(in crate::workflow::actor) async fn preparation_delivery_finished(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
) -> Result<(), WorkflowCoordinatorError> {
    let should_finalize = {
        let Some(pending) = coordinator.pending.get_mut(&key.attempt_id) else {
            return Ok(());
        };
        if pending.owner != owner || pending.key != key {
            return Ok(());
        }
        pending.delivery_finished = true;
        pending.abort_status.is_some()
    };
    if should_finalize {
        super::abort::enqueue_abort_finalization(coordinator, owner, key);
    }
    Ok(())
}
