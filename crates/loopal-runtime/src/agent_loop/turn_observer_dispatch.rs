use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use loopal_turn::{CancelCause, ToolExecState};
use tracing::{info, warn};

use super::governance::aggregator::AggregatedVerdict;
use super::governance::bridge::DataPlaneBridge;
use super::governance::system_note::make_governance_feedback;
use super::runner::AgentLoopRunner;
use super::turn_context::TurnContext;

impl AgentLoopRunner {
    pub(super) async fn run_before_tools(
        &mut self,
        turn_ctx: &mut TurnContext,
        tool_uses: &[(String, String, serde_json::Value)],
    ) -> Result<bool> {
        let verdicts: Vec<_> = self
            .governance
            .iter_mut()
            .map(|g| g.on_before_tools(turn_ctx, tool_uses))
            .collect();
        match self.aggregator.aggregate(verdicts) {
            AggregatedVerdict::Continue => Ok(false),
            AggregatedVerdict::Warnings(warnings) => {
                turn_ctx.pending_warnings.extend(warnings);
                Ok(false)
            }
            AggregatedVerdict::Abort {
                reason,
                feedback_to_model,
            } => {
                warn!(%reason, "governance aborted turn");
                self.emit_in_turn(AgentEventPayload::Error {
                    message: reason.clone(),
                })
                .await?;
                self.write_abort_compensation(tool_uses, &feedback_to_model);
                Ok(true)
            }
        }
    }

    /// Pair the assistant's tool_use blocks (already persisted in the
    /// preceding `LlmCall` step) with a `ToolBatch` step whose items are
    /// all `Cancelled(GovernanceAbort)`. Without this the wire would emit
    /// `tool_use[N]` without matching `tool_result[N]` and Anthropic would
    /// reject the next request.
    fn write_abort_compensation(
        &mut self,
        tool_uses: &[(String, String, serde_json::Value)],
        feedback_to_model: &str,
    ) {
        if !tool_uses.is_empty() {
            let count = tool_uses.len();
            match self.start_tool_batch_record(tool_uses) {
                Ok(Some(_step_index)) => {
                    for idx in 0..count {
                        self.update_tool_batch_item_state(
                            idx as u32,
                            ToolExecState::Cancelled(CancelCause::GovernanceAbort),
                        );
                    }
                    self.close_tool_batch_record();
                    info!(count, "abort compensation written");
                }
                Ok(None) => {}
                Err(e) => warn!(error = %e, "abort compensation start_tool_batch failed"),
            }
        }
        if let Some(note) = make_governance_feedback(feedback_to_model) {
            DataPlaneBridge::push_system_note(self, note);
        }
    }

    pub(super) async fn run_stop_hooks(&self) -> Option<String> {
        let stop_outputs = self
            .params
            .deps
            .kernel
            .hook_service()
            .run_hooks(
                loopal_config::HookEvent::Stop,
                &loopal_hooks::HookContext {
                    stop_reason: Some("end_turn"),
                    ..Default::default()
                },
            )
            .await;
        let feedback: Vec<&str> = stop_outputs
            .iter()
            .filter_map(|o| o.additional_context.as_deref())
            .collect();
        if feedback.is_empty() {
            None
        } else {
            Some(feedback.join("\n"))
        }
    }
}
