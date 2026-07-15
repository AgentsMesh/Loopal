use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::ContentBlock;
use loopal_tool_invocation::{CancelCause, ToolResultMetadata};
use loopal_turn::{CancelCause as TurnCancelCause, ToolExecState};
use tracing::info;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    // reason: 单点写回 tool_result event + ContentBlock，保证 view-state 与 LLM 同源
    // (历史上分两步调用导致 4 个 intercept handler 里 3 个漏 emit)。
    pub(super) async fn emit_and_block(
        &self,
        id: &str,
        name: &str,
        content: impl Into<String>,
        is_error: bool,
        metadata: Option<ToolResultMetadata>,
    ) -> Result<ContentBlock> {
        let content = content.into();
        self.emit_in_turn(AgentEventPayload::ToolResult {
            id: id.to_string(),
            name: name.to_string(),
            result: content.clone(),
            is_error,
            duration_ms: None,
            metadata: metadata.clone(),
        })
        .await?;
        Ok(ContentBlock::ToolResult {
            tool_use_id: id.to_string(),
            content,
            images: Vec::new(),
            is_error,
            metadata,
        })
    }

    /// Emit interrupted results for all tools (early cancel path).
    pub(super) async fn emit_all_interrupted(
        &mut self,
        tool_uses: &[(String, String, serde_json::Value)],
    ) -> Result<()> {
        info!("cancelled, skipping tool execution");
        let cancel_md = ToolResultMetadata::cancelled(CancelCause::UserInterrupt);
        for (id, name, _) in tool_uses {
            self.emit_and_block(
                id,
                name,
                "Interrupted by user",
                true,
                Some(cancel_md.clone()),
            )
            .await?;
        }
        if self.turns.current_tool_batch_step().is_some() {
            for (item_index, _) in tool_uses.iter().enumerate() {
                self.update_tool_batch_item_state(
                    item_index as u32,
                    ToolExecState::Cancelled(TurnCancelCause::UserInterrupt),
                );
            }
            self.close_tool_batch_record();
        }
        Ok(())
    }

    pub async fn inject_pending_messages(&mut self) {
        let pending = self.params.deps.frontend.drain_pending().await;
        for input in pending {
            match input {
                crate::agent_input::AgentInput::Message(env) => {
                    info!(
                        text_len = env.content.text.len(),
                        "injecting pending message"
                    );
                    self.ingest_message(&env).await;
                }
                crate::agent_input::AgentInput::Control(cmd) => {
                    if let Err(e) = self.apply_untracked_control(cmd).await {
                        tracing::warn!(error = %e, "failed to handle drained control");
                    }
                }
                crate::agent_input::AgentInput::TrackedControl(request) => {
                    if let Err(e) = self.apply_tracked_control(request).await {
                        tracing::warn!(error = %e, "failed to handle tracked control");
                    }
                }
            }
        }
    }
}
