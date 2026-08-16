use loopal_protocol::{WorkflowRunState, WorkflowTerminalDeliveryId, WorkflowTerminalDisposition};
use loopal_turn::TurnTrigger;

use super::runner::AgentLoopRunner;
use super::workflow_terminal_match::PersistedDelivery;
use crate::agent_input::WorkflowTerminalRequest;

impl AgentLoopRunner {
    pub(super) async fn apply_workflow_terminal(
        &mut self,
        request: WorkflowTerminalRequest,
    ) -> bool {
        let notification = request.notification();
        let rejection = notification
            .validate()
            .err()
            .map(|error| error.to_string())
            .or_else(|| {
                (notification.delivery_id.session_id != self.params.session.id)
                    .then(|| "workflow terminal delivery targets a different session".into())
            });
        if let Some(reason) = rejection {
            request
                .acknowledge(WorkflowTerminalDisposition::Rejected { reason })
                .await;
            return false;
        }

        let payload_digest = notification.payload_digest();
        let run_id = notification.delivery_id.run_id.clone();
        match self.restore_persisted_workflow_delivery(&notification.delivery_id, &payload_digest) {
            Ok(PersistedDelivery::Exact { should_execute }) => {
                request
                    .acknowledge(WorkflowTerminalDisposition::AlreadyApplied)
                    .await;
                self.params.workflow_lease_tracker.complete(&run_id);
                return should_execute;
            }
            Ok(PersistedDelivery::Conflict) => {
                request
                    .acknowledge(WorkflowTerminalDisposition::Rejected {
                        reason: "workflow terminal delivery id conflicts with persisted payload"
                            .into(),
                    })
                    .await;
                return false;
            }
            Ok(PersistedDelivery::Absent) => {}
            Err(error) => {
                request
                    .acknowledge(WorkflowTerminalDisposition::Retryable {
                        reason: format!("failed to inspect persisted turns: {error}"),
                    })
                    .await;
                tracing::warn!(
                    error = %error,
                    "workflow terminal persisted-turn inspection failed; retry queued"
                );
                return false;
            }
        }

        let trigger = workflow_trigger(notification);
        if self.start_durable_turn_record(trigger).is_none() {
            request
                .acknowledge(WorkflowTerminalDisposition::Retryable {
                    reason: "failed to durably persist workflow result turn".into(),
                })
                .await;
            return false;
        }
        request
            .acknowledge(WorkflowTerminalDisposition::Applied)
            .await;
        self.params.workflow_lease_tracker.complete(&run_id);
        true
    }

    fn restore_persisted_workflow_delivery(
        &mut self,
        delivery_id: &WorkflowTerminalDeliveryId,
        payload_digest: &str,
    ) -> loopal_error::Result<PersistedDelivery> {
        let events = self
            .params
            .deps
            .session_manager
            .load_turn_events(&self.params.session.id)?;
        let current = self.turns.store().turns();
        let current_classification =
            super::workflow_terminal_match::classify(&events, current, delivery_id, payload_digest);
        match current_classification {
            PersistedDelivery::Absent | PersistedDelivery::Conflict => {
                return Ok(current_classification);
            }
            PersistedDelivery::Exact { should_execute }
                if !should_execute
                    && super::workflow_terminal_match::contains_exact(
                        current,
                        delivery_id,
                        payload_digest,
                    ) =>
            {
                self.params
                    .deps
                    .session_manager
                    .sync_turn_events(&self.params.session.id)?;
                return Ok(PersistedDelivery::Exact {
                    should_execute: false,
                });
            }
            PersistedDelivery::Exact { .. } => {}
        }
        let turns = self
            .params
            .deps
            .session_manager
            .load_turns(&self.params.session.id)?;
        let classification =
            super::workflow_terminal_match::classify(&events, &turns, delivery_id, payload_digest);
        let PersistedDelivery::Exact { should_execute } = classification else {
            return Ok(classification);
        };
        self.params
            .deps
            .session_manager
            .sync_turn_events(&self.params.session.id)?;
        if should_execute {
            self.turns
                .replace_store(loopal_context::TurnStore::from_turns(turns));
        }
        Ok(classification)
    }
}

fn workflow_trigger(notification: &loopal_protocol::WorkflowTerminalNotification) -> TurnTrigger {
    TurnTrigger::WorkflowResult {
        session_id: notification.delivery_id.session_id.clone(),
        run_id: notification.delivery_id.run_id.to_string(),
        terminal_revision: notification.delivery_id.terminal_revision,
        payload_digest: notification.payload_digest(),
        state: state_name(notification.state).into(),
        content: notification.content.clone(),
    }
}

fn state_name(state: WorkflowRunState) -> &'static str {
    match state {
        WorkflowRunState::Succeeded => "succeeded",
        WorkflowRunState::Failed => "failed",
        WorkflowRunState::Cancelled => "cancelled",
        WorkflowRunState::Planned
        | WorkflowRunState::Validated
        | WorkflowRunState::Running
        | WorkflowRunState::Cancelling => "non_terminal",
    }
}

#[cfg(test)]
#[path = "input_workflow_terminal_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "input_workflow_terminal_failure_tests.rs"]
mod failure_tests;
