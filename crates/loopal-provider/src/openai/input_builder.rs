use loopal_message::{ContentBlock, MessageRole};
use loopal_provider_api::ChatParams;
use serde_json::{Value, json};

use super::OpenAiProvider;
use super::server_tool;

impl OpenAiProvider {
    /// Convert conversation messages into Responses API `input` array items.
    pub fn build_input(&self, params: &ChatParams) -> Vec<Value> {
        let mut input = Vec::new();

        for msg in &params.messages {
            match msg.role {
                MessageRole::System => {
                    // System messages go to `instructions`, skip here
                }
                MessageRole::User => {
                    let content = build_user_content(&msg.content);
                    if !content.is_empty() {
                        input.push(json!({
                            "type": "message",
                            "role": "user",
                            "content": content
                        }));
                    }
                    // Tool results from user messages → function_call_output items
                    for block in &msg.content {
                        if let ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } = block
                        {
                            input.push(json!({
                                "type": "function_call_output",
                                "call_id": tool_use_id,
                                "output": if *is_error {
                                    format!("[error] {content}")
                                } else {
                                    content.clone()
                                }
                            }));
                        }
                    }
                }
                MessageRole::Assistant => {
                    build_assistant_items(&msg.content, &mut input);
                }
            }
        }

        input
    }

    /// Build the `tools` array for the Responses API.
    pub fn build_tools(&self, params: &ChatParams) -> Vec<Value> {
        let mut tools = Vec::new();

        for tool in &params.tools {
            if tool.name == server_tool::WEB_SEARCH_TOOL_NAME {
                tools.push(server_tool::web_search_tool_definition());
            } else {
                tools.push(json!({
                    "type": "function",
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.input_schema
                }));
            }
        }

        tools
    }
}

/// Build content items for a user message (text + images only).
fn build_user_content(blocks: &[ContentBlock]) -> Vec<Value> {
    blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(json!({"type": "input_text", "text": text})),
            // Responses API uses data URL string (not the {url, detail} object of Chat Completions)
            ContentBlock::Image { source } => Some(json!({
                "type": "input_image",
                "image_url": format!("data:{};base64,{}", source.media_type, source.data)
            })),
            _ => None,
        })
        .collect()
}

/// Convert assistant content blocks into Responses API input items.
fn build_assistant_items(blocks: &[ContentBlock], input: &mut Vec<Value>) {
    let mut text_parts: Vec<Value> = Vec::new();

    for block in blocks {
        match block {
            ContentBlock::Text { text } => {
                text_parts.push(json!({"type": "output_text", "text": text}));
            }
            ContentBlock::ToolUse {
                id,
                name,
                input: args,
            } => {
                // Flush text before tool call
                flush_assistant_text(&mut text_parts, input);
                input.push(json!({
                    "type": "function_call",
                    "call_id": id,
                    "name": name,
                    "arguments": args.to_string()
                }));
            }
            ContentBlock::ServerToolUse {
                id,
                name: _,
                input: args,
            } => {
                flush_assistant_text(&mut text_parts, input);
                input.push(json!({
                    "type": "web_search_call",
                    "id": id,
                    "status": "completed",
                    "action": {"type": "search", "query": args.get("query").and_then(|v| v.as_str()).unwrap_or("")}
                }));
            }
            ContentBlock::Thinking { .. }
            | ContentBlock::ServerToolResult { .. }
            | ContentBlock::Image { .. }
            | ContentBlock::ToolResult { .. } => {}
        }
    }

    flush_assistant_text(&mut text_parts, input);
}

/// Flush accumulated text parts as an assistant message.
fn flush_assistant_text(text_parts: &mut Vec<Value>, input: &mut Vec<Value>) {
    if text_parts.is_empty() {
        return;
    }
    input.push(json!({
        "type": "message",
        "role": "assistant",
        "content": std::mem::take(text_parts)
    }));
}
