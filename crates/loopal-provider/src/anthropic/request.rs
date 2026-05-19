use loopal_message::{ContentBlock, MessageRole};
use loopal_provider_api::ChatParams;
use serde_json::{Value, json};
use tracing::error;

use super::AnthropicProvider;
use super::server_tool;

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
                            images,
                            is_error,
                            ..
                        } => {
                            let content_value = if images.is_empty() {
                                json!(content)
                            } else {
                                let mut blocks: Vec<Value> = Vec::new();
                                if !content.is_empty() {
                                    blocks.push(json!({"type": "text", "text": content}));
                                }
                                for img in images {
                                    let Some((media_type, data)) = img.as_inline() else {
                                        error!(
                                            media_type = img.media_type(),
                                            "SessionResource reached Anthropic provider without hydration; dropping image"
                                        );
                                        continue;
                                    };
                                    blocks.push(json!({
                                        "type": "image",
                                        "source": {
                                            "type": "base64",
                                            "media_type": media_type,
                                            "data": data
                                        }
                                    }));
                                }
                                json!(blocks)
                            };
                            json!({
                                "type": "tool_result",
                                "tool_use_id": tool_use_id,
                                "content": content_value,
                                "is_error": is_error
                            })
                        }
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
                        ContentBlock::ServerToolUse { id, name, input } => json!({
                            "type": "server_tool_use",
                            "id": id,
                            "name": name,
                            "input": input
                        }),
                        ContentBlock::ServerToolResult {
                            block_type,
                            tool_use_id,
                            content,
                        } => json!({
                            "type": block_type,
                            "tool_use_id": tool_use_id,
                            "content": content
                        }),
                    })
                    .collect();

                json!({ "role": role, "content": content })
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
        let mut tools: Vec<Value> = Vec::new();
        let mut last_client_idx: Option<usize> = None;

        for tool in &params.tools {
            if tool.name == server_tool::WEB_SEARCH_TOOL_NAME {
                // Replace client-side WebSearch with server-side declaration
                tools.push(server_tool::web_search_tool_definition(&params.model));
            } else {
                last_client_idx = Some(tools.len());
                tools.push(json!({
                    "name": tool.name,
                    "description": tool.description,
                    "input_schema": tool.input_schema
                }));
            }
        }

        // Place cache_control on the last client-side tool (not server tools)
        if let Some(idx) = last_client_idx {
            tools[idx]["cache_control"] = json!({"type": "ephemeral"});
        }

        tools
    }
}
