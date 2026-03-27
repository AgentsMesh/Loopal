use loopal_error::LoopalError;
use loopal_provider_api::{StopReason, StreamChunk};
use serde_json::Value;

/// Accumulates partial tool-use JSON fragments across streamed deltas.
#[derive(Default)]
pub(crate) struct ToolUseAccumulator {
    pub(crate) current_tool_id: Option<String>,
    pub(crate) current_tool_name: Option<String>,
    pub(crate) json_fragments: String,
    pub(crate) stop_reason: Option<StopReason>,
}

/// Accumulates thinking content across streamed deltas.
#[derive(Default)]
pub(crate) struct ThinkingAccumulator {
    pub(crate) active: bool,
    pub(crate) signature_fragments: String,
}

/// Accumulates server-side tool blocks (e.g. web_search, code_execution) across streamed events.
#[derive(Default)]
pub(crate) struct ServerToolAccumulator {
    /// Active server tool use: (id, name, input).
    pub(crate) current: Option<(String, String, serde_json::Value)>,
    /// Whether current block is a result (vs. tool_use).
    pub(crate) is_result: bool,
    /// For result blocks, the associated tool_use_id.
    pub(crate) result_tool_use_id: Option<String>,
    /// Raw content Value captured from `content_block_start` for result blocks.
    pub(crate) result_content: Option<serde_json::Value>,
    /// The original block type string, e.g. "web_search_tool_result".
    pub(crate) result_block_type: Option<String>,
    /// Accumulates `input_json_delta` fragments for server tools (e.g. code_execution).
    pub(crate) json_fragments: String,
}

impl ServerToolAccumulator {
    /// Whether a server tool use block is actively being streamed (not a result block).
    pub(crate) fn is_tool_use_active(&self) -> bool {
        self.current.is_some() && !self.is_result
    }
}

/// Extract usage tokens from a JSON object and push a `StreamChunk::Usage`.
pub(crate) fn push_usage_from(usage: &Value, chunks: &mut Vec<Result<StreamChunk, LoopalError>>) {
    let (Some(inp), Some(out)) = (
        usage["input_tokens"].as_u64(),
        usage["output_tokens"].as_u64(),
    ) else {
        return;
    };
    chunks.push(Ok(StreamChunk::Usage {
        input_tokens: inp as u32,
        output_tokens: out as u32,
        cache_creation_input_tokens: usage["cache_creation_input_tokens"].as_u64().unwrap_or(0)
            as u32,
        cache_read_input_tokens: usage["cache_read_input_tokens"].as_u64().unwrap_or(0) as u32,
        thinking_tokens: 0,
    }));
}
