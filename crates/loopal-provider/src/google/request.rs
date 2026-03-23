use loopal_message::{ContentBlock, MessageRole};
use loopal_provider_api::ChatParams;
use serde_json::{Value, json};

use super::GoogleProvider;

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

        let declarations: Vec<Value> = params
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.input_schema
                })
            })
            .collect();

        vec![json!({"functionDeclarations": declarations})]
    }
}
