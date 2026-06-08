use loopal_tool_invocation::ToolImageBlock;
use serde::de::{self, Deserializer};
use serde::ser::Serializer;
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

// reason: OpenAI Responses 要求每个 web_search_call 紧贴其 reasoning item；
// Anthropic 要求 thinking 块复现。reasoning 与 server tool 必须在同一序列里
// 保留 LLM 的流式发射顺序，单一 Option 装不下多个交错的 reasoning。
#[derive(Debug, Clone)]
pub enum ServerBlock {
    Reasoning(ThinkingBlock),
    ToolPair(ServerToolPair),
}

impl Serialize for ServerBlock {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            ServerBlock::Reasoning(t) => t.serialize(serializer),
            ServerBlock::ToolPair(p) => p.serialize(serializer),
        }
    }
}

// reason: 自定义 Deser 按 key 探测而非 #[serde(untagged)]，兼容旧 turns.jsonl
// 里 `{call,result}` 形状(直接读回 ToolPair)且错误信息可读。
impl<'de> Deserialize<'de> for ServerBlock {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.get("call").is_some() {
            serde_json::from_value(value)
                .map(ServerBlock::ToolPair)
                .map_err(de::Error::custom)
        } else if value.get("thinking").is_some() {
            serde_json::from_value(value)
                .map(ServerBlock::Reasoning)
                .map_err(de::Error::custom)
        } else {
            Err(de::Error::custom(
                "ServerBlock: expected `call` (ToolPair) or `thinking` (Reasoning) key",
            ))
        }
    }
}
