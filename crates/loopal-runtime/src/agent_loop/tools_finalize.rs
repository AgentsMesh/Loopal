use loopal_error::Result;
use loopal_provider_api::{ContentBlock, Message, MessageRole};
use loopal_turn::{
    OrderedToolBatch, ToolBatchItem, ToolCall, ToolCallId, ToolExecState, ToolResult, TurnStep,
};
use tracing::error;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub(super) fn finalize_tool_results(
        &mut self,
        mut indexed_results: Vec<(usize, ContentBlock)>,
    ) -> Result<()> {
        if indexed_results.is_empty() {
            return Ok(());
        }
        indexed_results.sort_by_key(|(idx, _)| *idx);
        let blocks: Vec<ContentBlock> = indexed_results.into_iter().map(|(_, b)| b).collect();
        let batch_step = build_tool_batch_step_from_blocks(&blocks);

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
        if let Some(step) = batch_step {
            self.append_step_record(step);
        }
        self.params.store.push_tool_results(msg);
        Ok(())
    }
}

fn build_tool_batch_step_from_blocks(blocks: &[ContentBlock]) -> Option<TurnStep> {
    // reason: tools_finalize 只拿到 ToolResult blocks (call 在 prev assistant 上)。
    // 用 tool_use_id 作为 ToolCall id 占位 — call.name/input 在 LlmCall step 已经
    // 持久化；这里 ToolBatch 主要承载 result 状态。后续 PR 让 tool_phase 把 call
    // 信息显式 thread 进来，消除占位字段。
    let items: Vec<ToolBatchItem> = blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
                ..
            } => Some(ToolBatchItem {
                call: ToolCall {
                    id: ToolCallId::new(tool_use_id),
                    name: String::new(),
                    input: serde_json::Value::Null,
                },
                state: ToolExecState::Done(ToolResult {
                    content: content.clone(),
                    is_error: *is_error,
                    images: vec![],
                }),
            }),
            _ => None,
        })
        .collect();
    if items.is_empty() {
        return None;
    }
    Some(TurnStep::ToolBatch(OrderedToolBatch { items }))
}
