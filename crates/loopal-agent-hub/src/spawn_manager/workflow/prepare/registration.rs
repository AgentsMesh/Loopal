use std::sync::Arc;

use super::super::{PreparationOwner, ProductionWorkflowSpawner};

pub(in crate::spawn_manager::workflow) struct PreparationRegistration {
    spawner: ProductionWorkflowSpawner,
    attempt: loopal_protocol::WorkflowAttemptId,
    preparation: Arc<PreparationOwner>,
    armed: bool,
}

impl PreparationRegistration {
    pub(in crate::spawn_manager::workflow) fn new(
        spawner: &ProductionWorkflowSpawner,
        attempt: loopal_protocol::WorkflowAttemptId,
        preparation: &Arc<PreparationOwner>,
    ) -> Self {
        Self {
            spawner: spawner.clone(),
            attempt,
            preparation: preparation.clone(),
            armed: true,
        }
    }

    pub(super) async fn remove(&mut self) {
        let mut owners = self.spawner.attempts.lock().await;
        remove_preparation(&mut owners, &self.attempt, &self.preparation);
        drop(owners);
        self.spawner.changed.notify_waiters();
        self.armed = false;
    }

    pub(super) fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for PreparationRegistration {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let spawner = self.spawner.clone();
        let attempt = self.attempt.clone();
        let preparation = self.preparation.clone();
        runtime.spawn(async move {
            let mut owners = spawner.attempts.lock().await;
            remove_preparation(&mut owners, &attempt, &preparation);
            drop(owners);
            spawner.changed.notify_waiters();
        });
    }
}

pub(super) fn remove_preparation(
    owners: &mut super::super::AttemptOwners,
    attempt: &loopal_protocol::WorkflowAttemptId,
    preparation: &Arc<PreparationOwner>,
) {
    if owners
        .preparing
        .get(attempt)
        .is_some_and(|current| Arc::ptr_eq(current, preparation))
    {
        owners.preparing.remove(attempt);
    }
}
