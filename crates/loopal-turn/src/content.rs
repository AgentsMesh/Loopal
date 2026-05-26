use loopal_tool_invocation::ToolImageBlock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ToolCallId(pub String);

impl ToolCallId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlock {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingBlock {
    pub thinking: String,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: ToolCallId,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: String,
    pub is_error: bool,
    /// Image attachments produced by the tool (e.g. Read on PNG/JPEG, screenshot
    /// tools). `Inline { media_type, data }` carries base64-encoded bytes;
    /// `SessionResource { id, media_type, byte_size }` references storage by
    /// id (hydrate to Inline before sending to LLM).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ToolImageBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerToolResult {
    pub block_type: String,
    pub content: serde_json::Value,
}

// reason: server tool 的 call 和 result 必须同 message 配对 (Anthropic I5).
// 用 struct 把两者绑死，不可能孤立存在。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerToolPair {
    pub call: ServerToolCall,
    pub result: ServerToolResult,
}
