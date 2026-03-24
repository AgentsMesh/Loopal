use loopal_message::{ContentBlock, MessageRole};
use loopal_provider_api::ChatParams;
use serde_json::{Value, json};

use super::GoogleProvider;
use super::server_tool;

impl GoogleProvider {
    pub fn build_contents(&self, params: &ChatParams) -> Vec<Value> {
        params
            .messages
            .iter()
            .filter(|m| m.role != MessageRole::System)
            .map(|msg| {
                let role = match msg.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "model",
                    MessageRole::System => unreachable!(),
                };

                let parts: Vec<Value> = msg
                    .content
                    .iter()
                    .map(|block| match block {
                        ContentBlock::Text { text } => json!({"text": text}),
                        ContentBlock::ToolUse { name, input, .. } => json!({
                            "functionCall": { "name": name, "args": input }
                        }),
                        ContentBlock::ToolResult {
                            tool_use_id: _,
                            content,
                            ..
                        } => json!({
                            "functionResponse": {
                                "name": "",
                                "response": {"result": content}
                            }
                        }),
                        ContentBlock::Image { source } => json!({
                            "inlineData": {
                                "mimeType": source.media_type,
                                "data": source.data
                            }
                        }),
                        ContentBlock::Thinking { thinking, .. } => json!({
                            "text": thinking,
                            "thought": true
                        }),
                        // Server-side blocks from other providers preserved as text.
                        // Content is formatted for readability when crossing providers.
                        ContentBlock::ServerToolUse { name, input, .. } => {
                            let query = input.get("query").and_then(|v| v.as_str()).unwrap_or("");
                            json!({"text": format!("[server tool: {name}({query})]")})
                        }
                        ContentBlock::ServerToolResult { content, .. } => {
                            let summary = summarize_search_result(content);
                            json!({"text": format!("[server tool result: {summary}]")})
                        }
                    })
                    .collect();

                json!({"role": role, "parts": parts})
            })
            .collect()
    }

    pub fn build_tools(&self, params: &ChatParams) -> Vec<Value> {
        if params.tools.is_empty() {
            return vec![];
        }

        let mut tools: Vec<Value> = Vec::new();
        let mut has_search = false;

        let declarations: Vec<Value> = params
            .tools
            .iter()
            .filter(|tool| {
                if tool.name == server_tool::WEB_SEARCH_TOOL_NAME {
                    has_search = true;
                    false // exclude from function declarations
                } else {
                    true
                }
            })
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.input_schema
                })
            })
            .collect();

        if !declarations.is_empty() {
            tools.push(json!({"functionDeclarations": declarations}));
        }
        if has_search {
            tools.push(server_tool::google_search_tool_definition());
        }

        tools
    }
}

/// Extract a concise summary from server-side search result JSON.
fn summarize_search_result(content: &serde_json::Value) -> String {
    if let Some(arr) = content.as_array() {
        let titles: Vec<&str> = arr
            .iter()
            .filter_map(|item| item.get("title").and_then(|v| v.as_str()))
            .take(3)
            .collect();
        if !titles.is_empty() {
            return titles.join(", ");
        }
    }
    let s = content.to_string();
    if s.len() <= 100 {
        return s;
    }
    // Safe truncation: find a char boundary at or before byte 97
    let mut end = 97;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &s[..end])
}
