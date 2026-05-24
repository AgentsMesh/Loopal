use loopal_provider_api::ContentBlock;
use loopal_turn::{
    AssistantOutput, ServerToolCall, ServerToolPair, ServerToolResult,
    StopReason as TurnStopReason, TextBlock, ThinkingBlock, ToolCall, ToolCallId, TurnStep,
};
use tracing::error;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Record the assistant response as an LlmCall step on the current turn.
    pub fn record_assistant_message(
        &mut self,
        assistant_text: &str,
        tool_uses: &[(String, String, serde_json::Value)],
        thinking_text: &str,
        thinking_signature: Option<&str>,
        server_blocks: Vec<ContentBlock>,
    ) {
        let has_thinking = thinking_signature.is_some()
            && (!thinking_text.is_empty() || !server_blocks.is_empty());
        let has_text = !assistant_text.is_empty();
        let has_tools = !tool_uses.is_empty();
        let has_server = !server_blocks.is_empty();
        if !has_thinking && !has_text && !has_tools && !has_server {
            return;
        }
        let step = build_llm_call_step(
            self.params.config.model(),
            assistant_text,
            tool_uses,
            thinking_text,
            thinking_signature,
            &server_blocks,
        );
        if let Err(e) = self.append_step_record(step) {
            error!(error = %e, "append_step(LlmCall) failed");
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
    let stop_reason = if tool_uses.is_empty() {
        TurnStopReason::EndTurn
    } else {
        TurnStopReason::ToolUse
    };
    TurnStep::LlmCall {
        model: model.to_string(),
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
