use loopal_provider_api::ContentBlock;
use loopal_provider_api::StopReason;

/// Structured result from `stream_llm_with()`, replacing the previous 4-element tuple.
pub struct LlmStreamResult {
    pub assistant_text: String,
    pub tool_uses: Vec<(String, String, serde_json::Value)>,
    /// True when the stream did not complete normally: explicit error chunk,
    /// user cancellation, or silent truncation (EOF without `message_stop`).
    /// `turn_exec` uses this to decide whether to auto-continue or bail.
    pub stream_error: bool,
    pub stop_reason: StopReason,
    pub thinking_text: String,
    pub thinking_tokens: u32,
    /// Reasoning + server-side tool blocks (Thinking / web_search), preserved in
    /// stream order so projection replays reasoning adjacent to its web_search_call.
    pub server_blocks: Vec<ContentBlock>,
}

impl Default for LlmStreamResult {
    fn default() -> Self {
        Self {
            assistant_text: String::new(),
            tool_uses: Vec::new(),
            stream_error: false,
            stop_reason: StopReason::EndTurn,
            thinking_text: String::new(),
            thinking_tokens: 0,
            server_blocks: Vec::new(),
        }
    }
}

impl LlmStreamResult {
    // reason: ThinkingSignature 把 thinking_text mem::take 进 server_blocks，故 metrics
    // 必须数 server_blocks 里的 Thinking 块；残留 thinking_text(未配 signature)算 1。
    pub fn thinking_block_count(&self) -> u32 {
        let in_blocks = self
            .server_blocks
            .iter()
            .filter(|b| matches!(b, ContentBlock::Thinking { .. }))
            .count() as u32;
        in_blocks + u32::from(!self.thinking_text.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thinking(id: &str) -> ContentBlock {
        ContentBlock::Thinking {
            thinking: id.into(),
            signature: Some(id.into()),
        }
    }

    #[test]
    fn counts_thinking_blocks_not_other_server_blocks() {
        let r = LlmStreamResult {
            server_blocks: vec![
                thinking("a"),
                ContentBlock::ServerToolUse {
                    id: "w".into(),
                    name: "web_search".into(),
                    input: serde_json::json!({}),
                },
                thinking("b"),
            ],
            ..Default::default()
        };
        assert_eq!(r.thinking_block_count(), 2);
    }

    #[test]
    fn counts_residual_unsigned_thinking_text_as_one() {
        let r = LlmStreamResult {
            thinking_text: "partial".into(),
            ..Default::default()
        };
        assert_eq!(r.thinking_block_count(), 1);
    }

    #[test]
    fn empty_response_counts_zero() {
        assert_eq!(LlmStreamResult::default().thinking_block_count(), 0);
    }
}
