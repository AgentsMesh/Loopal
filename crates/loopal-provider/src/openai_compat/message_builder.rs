use loopal_provider_api::{ChatParams, ContentBlock, Message, MessageRole};
use serde_json::{Value, json};
use tracing::error;

use super::OpenAiCompatProvider;
use crate::tool_result_text::placeholder_text;

impl OpenAiCompatProvider {
    pub fn build_messages_from_messages(
        &self,
        messages_in: &[Message],
        params: &ChatParams,
    ) -> Vec<Value> {
        let mut messages = Vec::new();

        if !params.system_prompt.is_empty() {
            messages.push(json!({
                "role": "system",
                "content": params.system_prompt
            }));
        }

        for msg in messages_in {
            match msg.role {
                MessageRole::System => {
                    messages.push(json!({
                        "role": "system",
                        "content": msg.text_content()
                    }));
                }
                MessageRole::User => {
                    let text = msg
                        .content
                        .iter()
                        .filter_map(|block| match block {
                            ContentBlock::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    let images = msg
                        .content
                        .iter()
                        .filter_map(|block| match block {
                            ContentBlock::Image { source } => Some(json!({
                                "type": "image_url",
                                "image_url": {"url": format!(
                                    "data:{};base64,{}", source.media_type, source.data
                                )}
                            })),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    push_user_content(&mut messages, &text, images);
                    for block in &msg.content {
                        if let ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            images,
                            ..
                        } = block
                        {
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
                                    push_user_content(&mut messages, "", parts);
                                }
                            }
                        }
                    }
                }
                MessageRole::Assistant => {
                    let mut content_text = String::new();
                    let mut reasoning_text = String::new();
                    let mut tool_calls = Vec::new();

                    for block in &msg.content {
                        match block {
                            ContentBlock::Text { text } => {
                                content_text.push_str(text);
                            }
                            ContentBlock::Thinking { thinking, .. } => {
                                reasoning_text.push_str(thinking);
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
                    if !reasoning_text.is_empty() {
                        assistant_msg["reasoning_content"] = json!(reasoning_text);
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

fn push_user_content(messages: &mut Vec<Value>, text: &str, mut images: Vec<Value>) {
    if images.is_empty() {
        if !text.is_empty() {
            messages.push(json!({"role": "user", "content": text}));
        }
        return;
    }
    if !text.is_empty() {
        images.insert(0, json!({"type": "text", "text": text}));
    }
    messages.push(json!({"role": "user", "content": images}));
}
