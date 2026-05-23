use loopal_provider_api::{ContentBlock, Message, MessageRole};
use loopal_turn::{
    AssistantOutput, LlmRequestSnapshot, ServerToolCall, ServerToolPair, ServerToolResult,
    StopReason as TurnStopReason, TextBlock, ThinkingBlock, ToolCall, ToolCallId, TurnStep,
};
use tracing::error;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Record the assistant response as a message in the conversation history.
    /// Writes to both persistent storage and in-memory store.
    /// Block order: thinking → server blocks → text → client tool_uses.
    pub fn record_assistant_message(
        &mut self,
        assistant_text: &str,
        tool_uses: &[(String, String, serde_json::Value)],
        thinking_text: &str,
        thinking_signature: Option<&str>,
        server_blocks: Vec<ContentBlock>,
    ) {
        let mut assistant_content: Vec<ContentBlock> = Vec::new();

        // Thinking block goes first (Anthropic API requires this order).
        // Skip if signature is missing — an unsigned thinking block (e.g. from
        // an interrupted stream) fails API validation on the next multi-turn call.
        // For OpenAI, signature stores the reasoning item ID — save even if text is empty.
        if thinking_signature.is_some() && (!thinking_text.is_empty() || !server_blocks.is_empty())
        {
            assistant_content.push(ContentBlock::Thinking {
                thinking: thinking_text.to_string(),
                signature: thinking_signature.map(String::from),
            });
        }

        // Server-side tool blocks (e.g. web_search) in stream order.
        for block in server_blocks.clone() {
            assistant_content.push(block);
        }

        if !assistant_text.is_empty() {
            assistant_content.push(ContentBlock::Text {
                text: assistant_text.to_string(),
            });
        }
        for (id, name, input) in tool_uses {
            assistant_content.push(ContentBlock::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
            });
        }

        if !assistant_content.is_empty() {
            let mut assistant_msg = Message {
                id: None,
                role: MessageRole::Assistant,
                content: assistant_content,
                origin: None,
                ephemeral_in_history: false,
            };
            if let Err(e) = self
                .params
                .deps
                .session_manager
                .save_message(&self.params.session.id, &mut assistant_msg)
            {
                error!(error = %e, "failed to persist message");
            }
            // Domain-layer mirror: record LlmCall step (parallel to message store).
            let step = build_llm_call_step(
                self.params.config.model(),
                assistant_text,
                tool_uses,
                thinking_text,
                thinking_signature,
                &server_blocks,
            );
            self.append_step_record(step);
            self.params.store.push_assistant(assistant_msg);
        }
    }
}

fn build_llm_call_step(
    model: &str,
    assistant_text: &str,
    tool_uses: &[(String, String, serde_json::Value)],
    thinking_text: &str,
    thinking_signature: Option<&str>,
    server_blocks: &[ContentBlock],
) -> TurnStep {
    let thinking = if thinking_signature.is_some() && !thinking_text.is_empty() {
        Some(ThinkingBlock {
            thinking: thinking_text.to_string(),
            signature: thinking_signature.map(String::from),
        })
    } else {
        None
    };
    let text_blocks = if assistant_text.is_empty() {
        vec![]
    } else {
        vec![TextBlock {
            text: assistant_text.to_string(),
        }]
    };
    let tool_calls: Vec<ToolCall> = tool_uses
        .iter()
        .map(|(id, name, input)| ToolCall {
            id: ToolCallId::new(id),
            name: name.clone(),
            input: input.clone(),
        })
        .collect();
    let server_pairs = pair_server_blocks(server_blocks);
    let tool_count = tool_calls.len() as u32;
    let stop_reason = if tool_uses.is_empty() {
        TurnStopReason::EndTurn
    } else {
        TurnStopReason::ToolUse
    };
    TurnStep::LlmCall {
        request_snapshot: LlmRequestSnapshot {
            model: model.to_string(),
            max_tokens: 0,
            tool_count,
            message_count: 0,
        },
        response: AssistantOutput {
            thinking,
            text_blocks,
            tool_calls,
            server_blocks: server_pairs,
            stop_reason,
        },
    }
}

fn pair_server_blocks(blocks: &[ContentBlock]) -> Vec<ServerToolPair> {
    let mut uses: std::collections::HashMap<String, (String, serde_json::Value)> =
        Default::default();
    let mut pairs = Vec::new();
    for b in blocks {
        if let ContentBlock::ServerToolUse { id, name, input } = b {
            uses.insert(id.clone(), (name.clone(), input.clone()));
        }
    }
    for b in blocks {
        if let ContentBlock::ServerToolResult {
            block_type,
            tool_use_id,
            content,
        } = b
            && let Some((name, input)) = uses.remove(tool_use_id)
        {
            pairs.push(ServerToolPair {
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
    pairs
}
