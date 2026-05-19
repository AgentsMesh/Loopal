use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use tracing::{info, warn};

use super::governance::aggregator::AggregatedVerdict;
use super::governance::bridge::DataPlaneBridge;
use super::governance::synthesize_aborted_tool_results;
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
                self.write_abort_compensation(tool_uses, &reason, &feedback_to_model);
                Ok(true)
            }
        }
    }

    fn write_abort_compensation(
        &mut self,
        tool_uses: &[(String, String, serde_json::Value)],
        reason: &str,
        feedback_to_model: &str,
    ) {
        if let Some(msg) = synthesize_aborted_tool_results(tool_uses, reason) {
            let count = tool_uses.len();
            DataPlaneBridge::write_tool_result_stub(self, msg);
            info!(count, "abort compensation written");
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
