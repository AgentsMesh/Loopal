use std::collections::HashMap;

use loopal_provider_api::ContentBlock;
use loopal_turn::{
    AssistantOutput, ServerBlock, ServerToolCall, ServerToolPair, ServerToolResult,
    StopReason as TurnStopReason, TextBlock, ThinkingBlock, ToolCall, ToolCallId, TurnStep,
};
use tracing::{error, warn};

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Record the assistant response as an LlmCall step on the current turn.
    pub fn record_assistant_message(
        &mut self,
        assistant_text: &str,
        tool_uses: &[(String, String, serde_json::Value)],
        server_blocks: Vec<ContentBlock>,
    ) {
        let has_text = !assistant_text.is_empty();
        let has_tools = !tool_uses.is_empty();
        // reason: reasoning(Thinking) 与 web_search 都在 server_blocks 里，非空即有内容
        let has_server = !server_blocks.is_empty();
        if !has_text && !has_tools && !has_server {
            warn!(
                "LLM returned an empty response (no text, tool_use, thinking, or \
                 server block); turn ends with no assistant output — check the \
                 provider/endpoint for dropped content on large or image requests"
            );
            return;
        }
        let step = build_llm_call_step(
            &self.params.config.model(),
            assistant_text,
            tool_uses,
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
    server_blocks: &[ContentBlock],
) -> TurnStep {
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
    let stop_reason = if tool_uses.is_empty() {
        TurnStopReason::EndTurn
    } else {
        TurnStopReason::ToolUse
    };
    TurnStep::LlmCall {
        model: model.to_string(),
        response: AssistantOutput {
            text_blocks,
            tool_calls,
            server_blocks: build_server_blocks(server_blocks),
            stop_reason,
        },
    }
}

// reason: 单趟按 Thinking/ServerToolUse 的流位置构造 ServerBlock，保留 reasoning 与
// web_search 的交错顺序（OpenAI 要求 reasoning 紧贴其 web_search_call）。result 按
// id 查表配对，容忍 use/result 非紧邻；孤立 use(截断响应)被丢弃。
fn build_server_blocks(blocks: &[ContentBlock]) -> Vec<ServerBlock> {
    let mut results: HashMap<&str, (&str, &serde_json::Value)> = HashMap::new();
    for b in blocks {
        if let ContentBlock::ServerToolResult {
            block_type,
            tool_use_id,
            content,
        } = b
        {
            results.insert(tool_use_id, (block_type, content));
        }
    }
    let mut out = Vec::new();
    for b in blocks {
        match b {
            ContentBlock::Thinking {
                thinking,
                signature,
            } => out.push(ServerBlock::Reasoning(ThinkingBlock {
                thinking: thinking.clone(),
                signature: signature.clone(),
            })),
            ContentBlock::ServerToolUse { id, name, input } => {
                if let Some((block_type, content)) = results.get(id.as_str()) {
                    out.push(ServerBlock::ToolPair(ServerToolPair {
                        call: ServerToolCall {
                            id: id.clone(),
                            name: name.clone(),
                            input: input.clone(),
                        },
                        result: ServerToolResult {
                            block_type: (*block_type).to_string(),
                            content: (*content).clone(),
                        },
                    }));
                }
            }
            _ => {}
        }
    }
    out
}
