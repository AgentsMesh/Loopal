use super::super::super::super::WorkflowCoordinator;
use crate::workflow::command::WorkflowCommand;
use crate::workflow::{WorkflowOwner, scheduler::AttemptKey};

pub(super) fn request_run(
    coordinator: &mut WorkflowCoordinator,
    owner: &WorkflowOwner,
    run_id: &loopal_protocol::WorkflowRunId,
) {
    let attempts: Vec<_> = coordinator
        .pending
        .values_mut()
        .filter(|pending| &pending.owner == owner && &pending.key.run_id == run_id)
        .filter_map(|pending| {
            if pending.abort_requested {
                return None;
            }
            pending.abort_requested = true;
            Some((pending.key.clone(), pending.causation.clone()))
        })
        .collect();
    for (key, causation) in attempts {
        start(coordinator, owner.clone(), key, causation);
    }
}

pub(super) fn request_attempt(
    coordinator: &mut WorkflowCoordinator,
    owner: &WorkflowOwner,
    key: &AttemptKey,
) {
    let Some(pending) = coordinator.pending.get_mut(&key.attempt_id) else {
        return;
    };
    if &pending.owner != owner || &pending.key != key || pending.abort_requested {
        return;
    }
    pending.abort_requested = true;
    let causation = pending.causation.clone();
    start(coordinator, owner.clone(), key.clone(), causation);
}

fn start(
    coordinator: &mut WorkflowCoordinator,
    owner: WorkflowOwner,
    key: AttemptKey,
    causation: loopal_protocol::WorkflowPermissionCausation,
) {
    let attempt_id = key.attempt_id.clone();
    let spawner = coordinator.spawner.clone();
    let callbacks = coordinator.callbacks.clone();
    let waiter = tokio::spawn(async move {
        let status = crate::workflow::scheduler::bounded_abort_prepare(spawner, &causation).await;
        send_completion(callbacks, owner, key, status).await;
        status
    });
    if let Some(pending) = coordinator.pending.get_mut(&attempt_id) {
        pending.abort_waiter = Some(waiter);
    }
}

async fn send_completion(
    callbacks: tokio::sync::mpsc::WeakSender<WorkflowCommand>,
    owner: WorkflowOwner,
    key: AttemptKey,
    status: crate::workflow::scheduler::WorkflowCleanupStatus,
) {
    let Some(callbacks) = callbacks.upgrade() else {
        return;
    };
    let _ = callbacks
        .send(WorkflowCommand::WorkerPreparationAborted { owner, key, status })
        .await;
}

#[cfg(test)]
mod tests {
    use loopal_protocol::{QualifiedAddress, WorkflowAttemptId, WorkflowNodeId, WorkflowRunId};
    use tokio::sync::{mpsc, oneshot};

    use super::*;

    #[tokio::test]
    async fn full_callback_queue_does_not_drop_abort_completion() {
        let (sender, mut receiver) = mpsc::channel(1);
        let (shutdown, _) = oneshot::channel();
        sender
            .send(WorkflowCommand::Shutdown { response: shutdown })
            .await
            .unwrap();
        let owner = WorkflowOwner::new("session", QualifiedAddress::local("root"));
        let key = AttemptKey {
            run_id: WorkflowRunId::new("wrun_queue_full"),
            node_id: WorkflowNodeId::new("wnode_queue_full"),
            attempt_id: WorkflowAttemptId::new("watt_queue_full"),
        };
        let callback = tokio::spawn(send_completion(
            sender.downgrade(),
            owner,
            key,
            crate::workflow::scheduler::WorkflowCleanupStatus::Confirmed,
        ));

        tokio::task::yield_now().await;
        assert!(!callback.is_finished());
        assert!(matches!(
            receiver.recv().await,
            Some(WorkflowCommand::Shutdown { .. })
        ));
        assert!(matches!(
            receiver.recv().await,
            Some(WorkflowCommand::WorkerPreparationAborted { .. })
        ));
        callback.await.unwrap();
    }
}
