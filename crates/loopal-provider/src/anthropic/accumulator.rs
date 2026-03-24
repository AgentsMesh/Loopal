use loopal_provider_api::StopReason;

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
}
