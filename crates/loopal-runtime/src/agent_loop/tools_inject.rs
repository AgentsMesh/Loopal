//! Helpers for tool interrupt handling, pending message injection, and result blocks.

use loopal_error::Result;
use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_protocol::AgentEventPayload;
use loopal_tool_invocation::{CancelCause, ToolResultMetadata};
use tracing::{error, info};

use super::runner::AgentLoopRunner;

pub(super) fn tool_result_block(
    id: &str,
    content: &str,
    is_error: bool,
    metadata: Option<ToolResultMetadata>,
) -> ContentBlock {
    ContentBlock::ToolResult {
        tool_use_id: id.to_string(),
        content: content.to_string(),
        is_error,
        metadata,
    }
}

impl AgentLoopRunner {
    /// Emit interrupted results for all tools (early cancel path).
    pub(super) async fn emit_all_interrupted(
        &mut self,
        tool_uses: &[(String, String, serde_json::Value)],
    ) -> Result<()> {
        info!("cancelled, skipping tool execution");
        let mut blocks = Vec::with_capacity(tool_uses.len());
        let cancel_md = ToolResultMetadata::cancelled(CancelCause::UserInterrupt);
        for (id, name, _) in tool_uses {
            self.emit_in_turn(AgentEventPayload::ToolResult {
                id: id.clone(),
                name: name.clone(),
                result: "Interrupted by user".into(),
                is_error: true,
                duration_ms: None,
                metadata: Some(cancel_md.clone()),
            })
            .await?;
            blocks.push(tool_result_block(
                id,
                "Interrupted by user",
                true,
                Some(cancel_md.clone()),
            ));
        }
        let mut msg = Message {
            id: None,
            role: MessageRole::User,
            content: blocks,
            origin: None,
        };
        if let Err(e) = self
            .params
            .deps
            .session_manager
            .save_message(&self.params.session.id, &mut msg)
        {
            error!(error = %e, "failed to persist message");
        }
        self.params.store.push_tool_results(msg);
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
                    if let Err(e) = self.handle_control(cmd).await {
                        tracing::warn!(error = %e, "failed to handle drained control");
                    }
                }
            }
        }
    }
}
