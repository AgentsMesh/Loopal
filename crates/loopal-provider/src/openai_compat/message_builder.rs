use loopal_provider_api::ChatParams;
use loopal_provider_api::{ContentBlock, MessageRole};
use serde_json::{Value, json};
use tracing::error;

use super::OpenAiCompatProvider;
use crate::tool_result_text::placeholder_text;

impl OpenAiCompatProvider {
    pub fn build_messages(&self, params: &ChatParams) -> Vec<Value> {
        let mut messages = Vec::new();

        if !params.system_prompt.is_empty() {
            messages.push(json!({
                "role": "system",
                "content": params.system_prompt
            }));
        }

        for msg in &params.messages {
            match msg.role {
                MessageRole::System => {
                    messages.push(json!({
                        "role": "system",
                        "content": msg.text_content()
                    }));
                }
                MessageRole::User => {
                    for block in &msg.content {
                        match block {
                            ContentBlock::Text { text } => {
                                messages.push(json!({
                                    "role": "user",
                                    "content": text
                                }));
                            }
                            ContentBlock::ToolResult {
                                tool_use_id,
                                content,
                                images,
                                ..
                            } => {
                                let placeholder = placeholder_text(content, !images.is_empty());
                                messages.push(json!({
                                    "role": "tool",
                                    "tool_call_id": tool_use_id,
                                    "content": placeholder
                                }));
                                if !images.is_empty() {
                                    let parts: Vec<Value> = images
                                        .iter()
                                        .filter_map(|img| match img.as_inline() {
                                            Some((media_type, data)) => Some(json!({
                                                "type": "image_url",
                                                "image_url": {
                                                    "url": format!(
                                                        "data:{};base64,{}",
                                                        media_type, data
                                                    )
                                                }
                                            })),
                                            None => {
                                                error!(
                                                    media_type = img.media_type(),
                                                    "SessionResource reached OpenAI-compat provider without hydration; dropping image"
                                                );
                                                None
                                            }
                                        })
                                        .collect();
                                    if !parts.is_empty() {
                                        messages.push(json!({
                                            "role": "user",
                                            "content": parts
                                        }));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                MessageRole::Assistant => {
                    let mut content_text = String::new();
                    let mut tool_calls = Vec::new();

                    for block in &msg.content {
                        match block {
                            ContentBlock::Text { text } => {
                                content_text.push_str(text);
                            }
                            ContentBlock::ToolUse { id, name, input } => {
                                tool_calls.push(json!({
                                    "id": id,
                                    "type": "function",
                                    "function": {
                                        "name": name,
                                        "arguments": input.to_string()
                                    }
                                }));
                            }
                            _ => {}
                        }
                    }

                    let mut assistant_msg = json!({"role": "assistant"});
                    if !content_text.is_empty() {
                        assistant_msg["content"] = json!(content_text);
                    }
                    if !tool_calls.is_empty() {
                        assistant_msg["tool_calls"] = json!(tool_calls);
                    }
                    messages.push(assistant_msg);
                }
            }
        }

        messages
    }

    pub fn build_tools(&self, params: &ChatParams) -> Vec<Value> {
        params
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.input_schema
                    }
                })
            })
            .collect()
    }
}
