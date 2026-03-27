//! Helpers for tool interrupt handling, pending message injection, and result blocks.

use loopal_error::Result;
use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_protocol::AgentEventPayload;
use tracing::{error, info};

use super::input::build_user_message;
use super::runner::AgentLoopRunner;

/// Build a successful ToolResult block.
pub(super) fn success_block(id: &str, content: &str) -> ContentBlock {
    ContentBlock::ToolResult {
        tool_use_id: id.to_string(),
        content: content.to_string(),
        is_error: false,
        is_completion: false,
        metadata: None,
    }
}

impl AgentLoopRunner {
    /// Emit interrupted results for all tools (early cancel path).
    pub(super) async fn emit_all_interrupted(
        &mut self,
        tool_uses: &[(String, String, serde_json::Value)],
    ) -> Result<Option<String>> {
        info!("cancelled, skipping tool execution");
        let mut blocks = Vec::with_capacity(tool_uses.len());
        for (id, name, _) in tool_uses {
            self.emit(AgentEventPayload::ToolResult {
                id: id.clone(),
                name: name.clone(),
                result: "Interrupted by user".into(),
                is_error: true,
                duration_ms: None,
                is_completion: false,
                metadata: None,
            })
            .await?;
            blocks.push(ContentBlock::ToolResult {
                tool_use_id: id.clone(),
                content: "Interrupted by user".into(),
                is_error: true,
                is_completion: false,
                metadata: None,
            });
        }
        let mut msg = Message {
            id: None,
            role: MessageRole::User,
            content: blocks,
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
        Ok(None)
    }

    /// Drain pending envelopes from the frontend and inject them as user messages.
    pub async fn inject_pending_messages(&mut self) {
        let pending = self.params.deps.frontend.drain_pending().await;
        for env in pending {
            let mut user_msg = build_user_message(&env);
            info!(
                text_len = env.content.text.len(),
                "injecting pending message"
            );
            if let Err(e) = self
                .params
                .deps
                .session_manager
                .save_message(&self.params.session.id, &mut user_msg)
            {
                error!(error = %e, "failed to persist injected message");
            }
            self.params.store.push_user(user_msg);
        }
    }
}
