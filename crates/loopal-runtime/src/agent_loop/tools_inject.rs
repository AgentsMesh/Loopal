//! Helpers for intercept handler completion, tool interrupt handling, pending
//! message injection, and result blocks.

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
        images: Vec::new(),
        is_error,
        metadata,
    }
}

impl AgentLoopRunner {
    // reason: emit ToolResult event + 构造 ContentBlock 是同一动作的两面 —— event 给
    // view-state, block 喂回 LLM。历史上 emit_tool_error / emit_tool_cancelled +
    // tool_result_block 分两步调用，4 个 intercept handler 里有 3 个漏 emit。此 helper
    // 统一两面入口, 任何 "工具结果回写" 路径走它即可保证两边一致。
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

    // reason: intercept handler 的薄包装 —— 在 emit_and_block 之上加 idx + turn_end_signal
    // 适配 intercept_special_tools 的聚合签名。详见 plans/breezy-tickling-cerf.md Part B1.
    pub(super) async fn complete_intercepted_tool(
        &self,
        idx: usize,
        id: &str,
        name: &str,
        content: impl Into<String>,
        is_error: bool,
        metadata: Option<ToolResultMetadata>,
        turn_end_signal: bool,
    ) -> Result<(usize, ContentBlock, bool)> {
        let block = self
            .emit_and_block(id, name, content, is_error, metadata)
            .await?;
        Ok((idx, block, turn_end_signal))
    }

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
            ephemeral_in_history: false,
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
