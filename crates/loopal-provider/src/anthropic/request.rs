use loopal_message::{ContentBlock, MessageRole};
use loopal_provider_api::ChatParams;
use serde_json::{Value, json};

use super::AnthropicProvider;

impl AnthropicProvider {
    pub fn build_messages(&self, params: &ChatParams) -> Vec<Value> {
        let mut messages: Vec<Value> = params
            .messages
            .iter()
            .filter(|m| m.role != MessageRole::System)
            .map(|msg| {
                let role = match msg.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::System => unreachable!(),
                };

                let content: Vec<Value> = msg
                    .content
                    .iter()
                    .map(|block| match block {
                        ContentBlock::Text { text } => json!({
                            "type": "text",
                            "text": text
                        }),
                        ContentBlock::ToolUse { id, name, input } => json!({
                            "type": "tool_use",
                            "id": id,
                            "name": name,
                            "input": input
                        }),
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } => json!({
                            "type": "tool_result",
                            "tool_use_id": tool_use_id,
                            "content": content,
                            "is_error": is_error
                        }),
                        ContentBlock::Image { source } => json!({
                            "type": "image",
                            "source": {
                                "type": source.source_type,
                                "media_type": source.media_type,
                                "data": source.data
                            }
                        }),
                        ContentBlock::Thinking {
                            thinking,
                            signature,
                        } => json!({
                            "type": "thinking",
                            "thinking": thinking,
                            "signature": signature.as_deref().unwrap_or("")
                        }),
                    })
                    .collect();

                json!({
                    "role": role,
                    "content": content
                })
            })
            .collect();

        // Cache breakpoint on last user message for multi-turn prompt caching.
        // Anthropic allows up to 4 breakpoints; system + last tool + this = 3.
        // Next turn, everything before this message hits cache_read (0.1x cost).
        if let Some(last_user) = messages.iter_mut().rev().find(|m| m["role"] == "user")
            && let Some(arr) = last_user["content"].as_array_mut()
            && let Some(last_block) = arr.last_mut()
        {
            last_block["cache_control"] = json!({"type": "ephemeral"});
        }

        messages
    }

    pub fn build_tools(&self, params: &ChatParams) -> Vec<Value> {
        let mut tools: Vec<Value> = params
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "input_schema": tool.input_schema
                })
            })
            .collect();

        // Place cache_control on the last tool for prompt caching
        if let Some(last) = tools.last_mut() {
            last["cache_control"] = json!({"type": "ephemeral"});
        }

        tools
    }
}
