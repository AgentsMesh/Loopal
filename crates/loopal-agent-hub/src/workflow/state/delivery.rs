use std::collections::HashSet;

use super::super::recovery::RecoveredOwner;
use super::super::{WorkflowCoordinatorError, WorkflowOwner};

pub(super) fn validate(
    owner: &WorkflowOwner,
    recovered: &RecoveredOwner,
) -> Result<(), WorkflowCoordinatorError> {
    let mut intent_ids = HashSet::new();
    for intent in &recovered.delivery_intents {
        let id = &intent.delivery_id;
        let exact_run = recovered.runs.iter().any(|run| {
            run.id == id.run_id
                && run.revision == id.terminal_revision
                && run.state == intent.state
                && run.state.is_terminal()
        });
        if intent.validate().is_err()
            || id.session_id != owner.session_id
            || !exact_run
            || !intent_ids.insert(id.clone())
        {
            return Err(WorkflowCoordinatorError::RecoveryConflict);
        }
    }
    if recovered
        .acked_deliveries
        .iter()
        .any(|delivery_id| !intent_ids.contains(delivery_id))
    {
        return Err(WorkflowCoordinatorError::RecoveryConflict);
    }
    Ok(())
}
