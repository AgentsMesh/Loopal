use loopal_error::Result;
use loopal_turn::{InjectionKind, TurnStep};
use tracing::{info, warn};

use super::runner::AgentLoopRunner;
use super::streaming_tool_exec::{self, StreamingToolHandle, ToolUseArrived};
use super::turn_context::TurnContext;

impl AgentLoopRunner {
    pub(super) async fn execute_tool_phase(
        &mut self,
        turn_ctx: &mut TurnContext,
        tool_uses: Vec<(String, String, serde_json::Value)>,
    ) -> Result<()> {
        self.update_fork_snapshot(&tool_uses);

        let kernel = std::sync::Arc::clone(&self.params.deps.kernel);
        let mut early_handle = StreamingToolHandle::empty();
        for (idx, (id, name, input)) in tool_uses.iter().enumerate() {
            streaming_tool_exec::feed_tool(
                &mut early_handle,
                &kernel,
                &self.tool_ctx,
                self.params.config.mode,
                &ToolUseArrived {
                    index: idx,
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                },
                self.params.deps.frontend.event_emitter(),
            );
        }

        let tool_names: Vec<&str> = tool_uses.iter().map(|(_, n, _)| n.as_str()).collect();
        info!(tool_count = tool_uses.len(), ?tool_names, "tool exec start");
        turn_ctx.metrics.tool_calls_requested += tool_uses.len() as u32;
        let stats = self
            .execute_tools(turn_ctx, tool_uses.clone(), early_handle)
            .await?;
        turn_ctx.metrics.tool_calls_approved += stats.approved;
        turn_ctx.metrics.tool_calls_denied += stats.denied;
        turn_ctx.metrics.tool_errors += stats.errors;
        info!("tool exec complete");

        // reason: capture result_blocks BEFORE BOTH the governance Injection
        // step append AND inject_pending_messages. Either would shift
        // `view.messages().last()` away from the tool batch's user message
        // — Injection projects to a User text message (the warning), and
        // inject_pending_messages may end the current turn entirely. If
        // we captured later, DiffTracker would see Text instead of
        // ToolResult and miss every Write/Edit/Apply/MultiEdit.
        let result_blocks: Vec<_> = self
            .turns
            .view()
            .messages()
            .last()
            .map(|m| m.content.clone())
            .unwrap_or_default();

        let warnings = std::mem::take(&mut turn_ctx.pending_warnings);
        let warning_count = warnings.len();
        turn_ctx.metrics.warnings_injected = warning_count as u32;
        if !warnings.is_empty() {
            let text = warnings.join("\n");
            if let Err(e) = self.append_step_record(TurnStep::Injection {
                kind: InjectionKind::SystemNote,
                text,
            }) {
                warn!(error = %e, "append_step(Injection) failed for governance warnings");
            }
        }

        self.inject_pending_messages().await;
        for h in &mut self.hooks {
            h.on_after_tools(turn_ctx, &tool_uses, &result_blocks);
        }
        Ok(())
    }
}
