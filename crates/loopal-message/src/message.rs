use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub role: MessageRole,
    pub content: Vec<ContentBlock>,
}

impl Message {
    pub fn user(text: &str) -> Self {
        Self {
            id: None,
            role: MessageRole::User,
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
        }
    }

    pub fn assistant(text: &str) -> Self {
        Self {
            id: None,
            role: MessageRole::Assistant,
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
        }
    }

    pub fn system(text: &str) -> Self {
        Self {
            id: None,
            role: MessageRole::System,
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
        }
    }

    /// Return a clone with the given ID assigned.
    pub fn with_id(mut self, id: String) -> Self {
        self.id = Some(id);
        self
    }

    pub fn text_content(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// Estimate the token count of this message across all content blocks.
    /// Uses a rough heuristic of 1 token per 4 characters.
    pub fn estimated_token_count(&self) -> u32 {
        let content_tokens: u32 = self
            .content
            .iter()
            .map(|block| match block {
                ContentBlock::Text { text } => text.len() as u32 / 4,
                ContentBlock::ToolUse { input, .. } => input.to_string().len() as u32 / 4,
                ContentBlock::ToolResult { content, .. } => content.len() as u32 / 4,
                ContentBlock::Image { .. } => 1000, // fixed estimate for images
                ContentBlock::Thinking { thinking, .. } => thinking.len() as u32 / 4,
                ContentBlock::ServerToolUse { input, .. } => input.to_string().len() as u32 / 4,
                ContentBlock::ServerToolResult { content, .. } => {
                    content.to_string().len() as u32 / 4
                }
            })
            .sum();
        // +4 for role/message overhead
        content_tokens + 4
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
    Image {
        source: ImageSource,
    },
    Thinking {
        thinking: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    /// Server-side tool invocation (preserved for multi-turn, not executed by client).
    ServerToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Server-side tool result (Anthropic/OpenAI/Google).
    /// `block_type` is the original API type string, e.g. "web_search_tool_result",
    /// "code_execution_tool_result". Used to serialize back the correct `"type"` field.
    ServerToolResult {
        block_type: String,
        tool_use_id: String,
        content: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub media_type: String,
    pub data: String,
}
