use loopal_protocol::{WorkflowRunId, WorkflowTerminalDeliveryId};

use super::super::error::WorkflowJournalError;

pub(super) fn id(
    expected_session: &str,
    expected_run: &WorkflowRunId,
    delivery_id: &WorkflowTerminalDeliveryId,
) -> Result<(), WorkflowJournalError> {
    delivery_id
        .validate()
        .map_err(|error| super::invalid(format!("invalid delivery id: {error:?}")))?;
    if delivery_id.session_id != expected_session {
        return Err(super::invalid(
            "delivery session does not match journal session",
        ));
    }
    super::run_id(expected_run, &delivery_id.run_id)
}
