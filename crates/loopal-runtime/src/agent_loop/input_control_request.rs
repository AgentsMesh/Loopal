use loopal_error::Result;
use loopal_protocol::ControlCommand;
use tracing::warn;

use super::input_control::ControlOutcome;
use super::runner::AgentLoopRunner;
use crate::agent_input::{ControlAcknowledgement, ControlRequest};

impl AgentLoopRunner {
    pub(super) async fn apply_untracked_control(
        &mut self,
        command: ControlCommand,
    ) -> Result<bool> {
        match self.handle_control(command).await? {
            ControlOutcome::Applied { continuation } => Ok(continuation),
            ControlOutcome::Rejected(reason) => {
                warn!(%reason, "control command rejected");
                Ok(false)
            }
        }
    }

    pub(super) async fn apply_tracked_control(&mut self, request: ControlRequest) -> Result<bool> {
        if !request.caller_is_waiting() {
            return Ok(false);
        }
        let outcome = self.handle_control(request.command().clone()).await;
        match outcome {
            Ok(ControlOutcome::Applied { continuation }) => {
                request.acknowledge(ControlAcknowledgement::Applied).await;
                Ok(continuation)
            }
            Ok(ControlOutcome::Rejected(reason)) => {
                request
                    .acknowledge(ControlAcknowledgement::Rejected(reason))
                    .await;
                Ok(false)
            }
            Err(error) => {
                request
                    .acknowledge(ControlAcknowledgement::Rejected(format!(
                        "runtime failed to apply control: {error}"
                    )))
                    .await;
                Err(error)
            }
        }
    }
}
