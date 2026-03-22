use loopal_provider_api::StopReason;

/// Structured result from `stream_llm_with()`, replacing the previous 4-element tuple.
pub struct LlmStreamResult {
    pub assistant_text: String,
    pub tool_uses: Vec<(String, String, serde_json::Value)>,
    pub stream_error: bool,
    pub stop_reason: StopReason,
    pub thinking_text: String,
    pub thinking_signature: Option<String>,
    pub thinking_tokens: u32,
}

impl Default for LlmStreamResult {
    fn default() -> Self {
        Self {
            assistant_text: String::new(),
            tool_uses: Vec::new(),
            stream_error: false,
            stop_reason: StopReason::EndTurn,
            thinking_text: String::new(),
            thinking_signature: None,
            thinking_tokens: 0,
        }
    }
}
