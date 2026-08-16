use loopal_error::Result;
use loopal_protocol::{AgentEventPayload, AgentStatus};
use tracing::info;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub(super) fn notify_observers_envelope_received(
        &mut self,
        source: &loopal_protocol::MessageSource,
    ) {
        for governance in &mut self.governance {
            governance.on_envelope_received(source);
        }
    }

    async fn emit_interrupted(&mut self) -> Result<()> {
        info!("agent interrupted by user");
        self.status = AgentStatus::WaitingForInput;
        self.emit(AgentEventPayload::Interrupted).await
    }

    // Finalize even if the Interrupted emit fails, so no turn stays open.
    pub(super) async fn collect_interrupted_turn(&mut self) -> Result<()> {
        let emit_result = self.emit_interrupted().await;
        self.finalize_turn_cancellation(loopal_turn::CancelledCause::UserInterrupt)
            .await;
        emit_result
    }

    pub async fn emit_inbox_consumed(&mut self) {
        let ids = std::mem::take(&mut self.pending_consumed_ids);
        for message_id in ids {
            if let Err(error) = self
                .emit(AgentEventPayload::InboxConsumed {
                    envelope_id: message_id.clone(),
                })
                .await
            {
                tracing::error!(
                    error = %error,
                    message_id = %message_id,
                    "agent_loop::run_events InboxConsumed emit failed"
                );
            }
        }
    }
}
