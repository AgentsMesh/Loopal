use super::super::WorkflowCoordinator;
use super::{recovery, stop};
use crate::workflow::WorkflowCoordinatorError;

impl WorkflowCoordinator {
    pub(in crate::workflow::actor) async fn handle_tick(
        &mut self,
        now_unix_ms: u64,
    ) -> Result<(), WorkflowCoordinatorError> {
        if let Some(error) = self.terminal_delivery_failure.clone() {
            return Err(error);
        }
        recovery::reconcile_expired(self, now_unix_ms).await?;
        stop::tick(self, now_unix_ms).await?;
        let owners: Vec<_> = self.resumed_owners.iter().cloned().collect();
        for owner in owners {
            self.resume_owner(owner).await?;
        }
        super::super::super::terminal_delivery::retry_all(self);
        Ok(())
    }
}
