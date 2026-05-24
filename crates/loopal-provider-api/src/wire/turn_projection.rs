use loopal_turn::{
    AssistantOutput, CompactionRehydrate, CompactionSummary, InjectionKind, OrderedToolBatch,
    ServerToolPair, TextBlock, ThinkingBlock, ToolBatchItem, ToolCall, ToolExecState, ToolResult,
    Turn, TurnStep, TurnTrigger,
};

use super::message::{ContentBlock, Message, MessageRole};
use super::origin::MessageOrigin;

pub fn project_turns_to_messages(turns: &[Turn]) -> Vec<Message> {
    turns.iter().flat_map(project_turn_to_messages).collect()
}

pub fn project_turn_to_messages(turn: &Turn) -> Vec<Message> {
    let mut out = Vec::new();
    if let Some(msg) = project_trigger(&turn.trigger) {
        out.push(msg);
    }
    for step in &turn.body.steps {
        out.extend(project_step(step));
    }
    out
}

fn project_trigger(trigger: &TurnTrigger) -> Option<Message> {
    // reason: 与 runtime/message_build::build_user_message 的前缀规则保持一致 —
    // 投影出的 user message 在 LLM 上下文中和 ingest 时直接写入的版本等价。
    match trigger {
        TurnTrigger::UserInput { content, .. } => {
            Some(text_user(content, Some(MessageOrigin::Human)))
        }
        TurnTrigger::Cron { content, .. } => Some(text_user(
            &format!("[scheduled] {content}"),
            Some(MessageOrigin::Scheduled),
        )),
        TurnTrigger::Agent { from, content, .. } => Some(text_user(
            &format!("[from: {from}] {content}"),
            Some(MessageOrigin::Agent {
                label: from.clone(),
            }),
        )),
        TurnTrigger::Channel {
            channel,
            from,
            content,
            ..
        } => Some(text_user(
            &format!("[from: #{channel}/{from}] {content}"),
            Some(MessageOrigin::Channel {
                name: channel.clone(),
                from: from.clone(),
            }),
        )),
        TurnTrigger::GoalContinuation { content, .. } => {
            Some(text_user(content, Some(MessageOrigin::GoalContinuation)))
        }
        TurnTrigger::BackgroundHook {
            hook_kind, content, ..
        } => Some(text_user(
            content,
            Some(MessageOrigin::Other {
                label: hook_kind.clone(),
            }),
        )),
        TurnTrigger::Resume => None,
    }
}

fn project_step(step: &TurnStep) -> Vec<Message> {
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
    // reason: Vec 顺序锁定 I4 invariant — items 顺序即 ToolCall 顺序，投影到的
    // ToolResult 顺序自然匹配 ToolUse 顺序。
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
        // reason: 投影到 wire format 时不应该出现未结束的 tool；缺省按 error 占位。
        ToolExecState::Pending | ToolExecState::Running => ContentBlock::ToolResult {
            tool_use_id,
            content: "Pending".to_string(),
            images: Vec::new(),
            is_error: true,
            metadata: None,
        },
    }
}

fn project_compaction_summary(s: &CompactionSummary) -> Vec<Message> {
    let summary = Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::Text {
            text: s.summary_text.clone(),
        }],
        origin: Some(MessageOrigin::CompactionSummary),
        ephemeral_in_history: false,
    };
    let ack = Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![ContentBlock::Text {
            text: s.ack_text.clone(),
        }],
        origin: Some(MessageOrigin::CompactionSummary),
        ephemeral_in_history: false,
    };
    vec![summary, ack]
}

fn project_compaction_rehydrate(r: &CompactionRehydrate) -> Vec<Message> {
    // reason: per file emit assistant tool_use(Read) + user tool_result(content)
    // pair, matching the original compact_rehydrate.rs serialization that the
    // LLM expects to see in conversation history.
    let mut out = Vec::with_capacity(r.files.len() * 2);
    for f in &r.files {
        out.push(Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: f.tool_call_id.as_str().to_string(),
                name: "Read".to_string(),
                input: serde_json::json!({ "file_path": f.path }),
            }],
            origin: Some(MessageOrigin::CompactionRehydrate),
            ephemeral_in_history: false,
        });
        out.push(Message {
            id: None,
            role: MessageRole::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: f.tool_call_id.as_str().to_string(),
                content: f.content.clone(),
                images: Vec::new(),
                is_error: false,
                metadata: None,
            }],
            origin: Some(MessageOrigin::CompactionRehydrate),
            ephemeral_in_history: false,
        });
    }
    out
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

fn text_user(text: &str, origin: Option<MessageOrigin>) -> Message {
    Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::Text {
            text: text.to_string(),
        }],
        origin,
        ephemeral_in_history: false,
    }
}

fn text_block(block: &TextBlock) -> ContentBlock {
    ContentBlock::Text {
        text: block.text.clone(),
    }
}

fn thinking_block(t: &ThinkingBlock) -> ContentBlock {
    ContentBlock::Thinking {
        thinking: t.thinking.clone(),
        signature: t.signature.clone(),
    }
}

fn tool_use_block(call: &ToolCall) -> ContentBlock {
    ContentBlock::ToolUse {
        id: call.id.as_str().to_string(),
        name: call.name.clone(),
        input: call.input.clone(),
    }
}

fn tool_result_block(tool_use_id: &str, r: &ToolResult) -> ContentBlock {
    ContentBlock::ToolResult {
        tool_use_id: tool_use_id.to_string(),
        content: r.content.clone(),
        images: r.images.clone(),
        is_error: r.is_error,
        metadata: None,
    }
}

fn server_pair_blocks(pair: &ServerToolPair) -> Vec<ContentBlock> {
    vec![
        ContentBlock::ServerToolUse {
            id: pair.call.id.clone(),
            name: pair.call.name.clone(),
            input: pair.call.input.clone(),
        },
        ContentBlock::ServerToolResult {
            block_type: pair.result.block_type.clone(),
            tool_use_id: pair.call.id.clone(),
            content: pair.result.content.clone(),
        },
    ]
}
