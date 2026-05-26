use loopal_turn::{
    AssistantOutput, InjectionKind, OrderedToolBatch, ToolBatchItem, ToolExecState, TurnStep,
};

use super::super::message::{ContentBlock, Message, MessageRole};
use super::super::origin::MessageOrigin;
use super::blocks::{
    server_pair_blocks, text_block, thinking_block, tool_result_block, tool_use_block,
};
use super::compaction::{project_compaction_rehydrate, project_compaction_summary};
use super::trigger::text_user;

pub(super) fn project_step(step: &TurnStep) -> Vec<Message> {
    match step {
        TurnStep::LlmCall { response, .. } => vec![project_assistant(response)],
        TurnStep::ToolBatch(batch) => project_tool_batch(batch).into_iter().collect(),
        TurnStep::CompactionSummary(s) => project_compaction_summary(s),
        TurnStep::CompactionRehydrate(r) => project_compaction_rehydrate(r),
        TurnStep::Injection { kind, text } => vec![project_injection(kind, text)],
    }
}

fn project_assistant(response: &AssistantOutput) -> Message {
    let mut content = Vec::new();
    if let Some(t) = &response.thinking {
        content.push(thinking_block(t));
    }
    for block in &response.text_blocks {
        content.push(text_block(block));
    }
    for call in &response.tool_calls {
        content.push(tool_use_block(call));
    }
    for pair in &response.server_blocks {
        content.extend(server_pair_blocks(pair));
    }
    Message {
        id: None,
        role: MessageRole::Assistant,
        content,
        origin: None,
        ephemeral_in_history: false,
    }
}

fn project_tool_batch(batch: &OrderedToolBatch) -> Option<Message> {
    if batch.items.is_empty() {
        return None;
    }
    let content: Vec<ContentBlock> = batch.items.iter().map(tool_batch_item_to_block).collect();
    Some(Message {
        id: None,
        role: MessageRole::User,
        content,
        origin: None,
        ephemeral_in_history: false,
    })
}

fn tool_batch_item_to_block(item: &ToolBatchItem) -> ContentBlock {
    let tool_use_id = item.call.id.as_str().to_string();
    match &item.state {
        ToolExecState::Done(r) => tool_result_block(&tool_use_id, r),
        ToolExecState::Cancelled(_) => ContentBlock::ToolResult {
            tool_use_id,
            content: "Cancelled".to_string(),
            images: Vec::new(),
            is_error: true,
            metadata: None,
        },
        ToolExecState::Pending | ToolExecState::Running => ContentBlock::ToolResult {
            tool_use_id,
            content: "Pending".to_string(),
            images: Vec::new(),
            is_error: true,
            metadata: None,
        },
    }
}

fn project_injection(kind: &InjectionKind, text: &str) -> Message {
    let origin = match kind {
        InjectionKind::Governance => MessageOrigin::GovernanceFeedback,
        InjectionKind::StopFeedback => MessageOrigin::StopFeedback,
        InjectionKind::ConfigRefresh => MessageOrigin::ConfigRefresh,
        InjectionKind::SystemNote => MessageOrigin::Other {
            label: "system_note".into(),
        },
    };
    text_user(text, Some(origin))
}
