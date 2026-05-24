use chrono::Utc;
use loopal_provider_api::{ContentBlock, Message, MessageRole};
use loopal_turn::{
    AssistantOutput, OrderedToolBatch, ServerToolCall, ServerToolPair, ServerToolResult,
    StopReason, TextBlock, ThinkingBlock, ToolBatchItem, ToolCall, ToolCallId, ToolExecState,
    ToolResult, Turn, TurnBody, TurnId, TurnOutcome, TurnStep, TurnTrigger,
};
use std::collections::HashMap;

/// Convert a flat `Vec<Message>` loaded from a pre-Turn `messages.jsonl` into
/// the new `Vec<Turn>` shape so a legacy resume seeds `TurnStore` with the
/// recovered history. Without this, the LLM would see an empty turn list on
/// the next request and forget every pre-resume exchange.
///
/// Mapping:
/// - User text → starts a fresh `Turn` with `TurnTrigger::UserInput`
/// - User tool_result-only → `TurnStep::ToolBatch` appended to current turn,
///   items paired with the previous assistant's ToolUse blocks by id
/// - Assistant → `TurnStep::LlmCall` appended to current turn
/// - Orphaned tool results (no preceding tool_use) become Cancelled items so
///   downstream invariants still hold
pub fn legacy_messages_to_turns(messages: Vec<Message>) -> Vec<Turn> {
    let mut turns: Vec<Turn> = Vec::new();
    let mut current: Option<Turn> = None;
    let mut pending_tool_calls: HashMap<String, ToolCall> = HashMap::new();

    for msg in messages {
        match msg.role {
            MessageRole::User => {
                if has_tool_result(&msg.content) {
                    let items = pair_tool_results(&msg.content, &mut pending_tool_calls);
                    if let Some(ref mut turn) = current {
                        turn.body
                            .steps
                            .push(TurnStep::ToolBatch(OrderedToolBatch { items }));
                    }
                } else {
                    if let Some(t) = current.take() {
                        turns.push(t);
                    }
                    current = Some(new_user_turn(msg));
                }
            }
            MessageRole::Assistant => {
                let response = build_assistant_output(&msg.content);
                for call in &response.tool_calls {
                    pending_tool_calls.insert(call.id.as_str().to_string(), call.clone());
                }
                let turn = current.get_or_insert_with(synthetic_resume_turn);
                turn.body.steps.push(TurnStep::LlmCall {
                    model: String::new(),
                    response,
                });
            }
            MessageRole::System => {}
        }
    }
    if let Some(t) = current {
        turns.push(t);
    }
    turns
}

fn synthetic_resume_turn() -> Turn {
    Turn {
        id: TurnId::new(),
        started_at: Utc::now(),
        trigger: TurnTrigger::Resume,
        body: TurnBody::default(),
        outcome: TurnOutcome::Complete,
    }
}

fn new_user_turn(msg: Message) -> Turn {
    let envelope_id = msg.id.clone().unwrap_or_default();
    let content = extract_text(&msg.content);
    let images = extract_images(&msg.content);
    Turn {
        id: TurnId::new(),
        started_at: Utc::now(),
        trigger: TurnTrigger::UserInput {
            envelope_id,
            content,
            images,
        },
        body: TurnBody::default(),
        outcome: TurnOutcome::Complete,
    }
}

fn has_tool_result(content: &[ContentBlock]) -> bool {
    content
        .iter()
        .any(|b| matches!(b, ContentBlock::ToolResult { .. }))
}

fn extract_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_images(content: &[ContentBlock]) -> Vec<loopal_tool_invocation::ToolImageBlock> {
    content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Image { source } => {
                Some(loopal_tool_invocation::ToolImageBlock::Inline {
                    media_type: source.media_type.clone(),
                    data: source.data.clone(),
                })
            }
            _ => None,
        })
        .collect()
}

fn pair_tool_results(
    content: &[ContentBlock],
    pending: &mut HashMap<String, ToolCall>,
) -> Vec<ToolBatchItem> {
    content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                images,
                is_error,
                ..
            } => {
                let call = pending.remove(tool_use_id).unwrap_or_else(|| ToolCall {
                    id: ToolCallId::new(tool_use_id),
                    name: "unknown".to_string(),
                    input: serde_json::Value::Null,
                });
                Some(ToolBatchItem {
                    call,
                    state: ToolExecState::Done(ToolResult {
                        content: content.clone(),
                        is_error: *is_error,
                        images: images.clone(),
                    }),
                })
            }
            _ => None,
        })
        .collect()
}

fn build_assistant_output(content: &[ContentBlock]) -> AssistantOutput {
    let mut thinking: Option<ThinkingBlock> = None;
    let mut text_blocks: Vec<TextBlock> = Vec::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();
    let mut server_calls: HashMap<String, (String, serde_json::Value)> = HashMap::new();
    let mut server_blocks: Vec<ServerToolPair> = Vec::new();

    for block in content {
        match block {
            ContentBlock::Thinking {
                thinking: t,
                signature,
            } => {
                thinking = Some(ThinkingBlock {
                    thinking: t.clone(),
                    signature: signature.clone(),
                });
            }
            ContentBlock::Text { text } => {
                text_blocks.push(TextBlock { text: text.clone() });
            }
            ContentBlock::ToolUse { id, name, input } => {
                tool_calls.push(ToolCall {
                    id: ToolCallId::new(id),
                    name: name.clone(),
                    input: input.clone(),
                });
            }
            ContentBlock::ServerToolUse { id, name, input } => {
                server_calls.insert(id.clone(), (name.clone(), input.clone()));
            }
            ContentBlock::ServerToolResult {
                block_type,
                tool_use_id,
                content,
            } => {
                if let Some((name, input)) = server_calls.remove(tool_use_id) {
                    server_blocks.push(ServerToolPair {
                        call: ServerToolCall {
                            id: tool_use_id.clone(),
                            name,
                            input,
                        },
                        result: ServerToolResult {
                            block_type: block_type.clone(),
                            content: content.clone(),
                        },
                    });
                }
            }
            ContentBlock::ToolResult { .. } | ContentBlock::Image { .. } => {}
        }
    }

    let stop_reason = if tool_calls.is_empty() {
        StopReason::EndTurn
    } else {
        StopReason::ToolUse
    };
    AssistantOutput {
        thinking,
        text_blocks,
        tool_calls,
        server_blocks,
        stop_reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_in_empty_out() {
        assert!(legacy_messages_to_turns(vec![]).is_empty());
    }

    #[test]
    fn single_user_msg_becomes_one_turn() {
        let msgs = vec![Message::user("hi")];
        let turns = legacy_messages_to_turns(msgs);
        assert_eq!(turns.len(), 1);
        match &turns[0].trigger {
            TurnTrigger::UserInput { content, .. } => assert_eq!(content, "hi"),
            other => panic!("expected UserInput, got {other:?}"),
        }
        assert!(turns[0].body.steps.is_empty());
    }

    #[test]
    fn user_assistant_pair_becomes_one_turn_with_llmcall() {
        let msgs = vec![Message::user("ask"), Message::assistant("reply")];
        let turns = legacy_messages_to_turns(msgs);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].body.steps.len(), 1);
        match &turns[0].body.steps[0] {
            TurnStep::LlmCall { response, .. } => {
                assert_eq!(response.text_blocks.len(), 1);
                assert_eq!(response.text_blocks[0].text, "reply");
            }
            other => panic!("expected LlmCall, got {other:?}"),
        }
    }

    #[test]
    fn tool_use_result_pair_round_trip() {
        let tool_use = ContentBlock::ToolUse {
            id: "tc-1".into(),
            name: "Read".into(),
            input: serde_json::json!({"file_path": "/x"}),
        };
        let tool_result = ContentBlock::ToolResult {
            tool_use_id: "tc-1".into(),
            content: "file body".into(),
            images: vec![],
            is_error: false,
            metadata: None,
        };
        let mut assistant = Message::assistant("");
        assistant.content = vec![tool_use];
        let mut user_result = Message::user("");
        user_result.content = vec![tool_result];
        let turns = legacy_messages_to_turns(vec![
            Message::user("read"),
            assistant,
            user_result,
            Message::assistant("done"),
        ]);
        assert_eq!(turns.len(), 1);
        let steps = &turns[0].body.steps;
        assert_eq!(steps.len(), 3);
        match &steps[0] {
            TurnStep::LlmCall { response, .. } => {
                assert_eq!(response.tool_calls.len(), 1);
                assert_eq!(response.tool_calls[0].name, "Read");
            }
            _ => panic!("step 0 must be LlmCall"),
        }
        match &steps[1] {
            TurnStep::ToolBatch(b) => {
                assert_eq!(b.items.len(), 1);
                assert_eq!(b.items[0].call.name, "Read");
                assert!(matches!(b.items[0].state, ToolExecState::Done(_)));
            }
            _ => panic!("step 1 must be ToolBatch"),
        }
    }

    #[test]
    fn orphaned_tool_result_uses_unknown_call_stub() {
        let tool_result = ContentBlock::ToolResult {
            tool_use_id: "orphan".into(),
            content: "x".into(),
            images: vec![],
            is_error: false,
            metadata: None,
        };
        let mut user_result = Message::user("");
        user_result.content = vec![tool_result];
        let turns = legacy_messages_to_turns(vec![Message::user("hi"), user_result]);
        assert_eq!(turns.len(), 1);
        match &turns[0].body.steps[0] {
            TurnStep::ToolBatch(b) => {
                assert_eq!(b.items[0].call.name, "unknown");
            }
            _ => panic!(),
        }
    }
}
