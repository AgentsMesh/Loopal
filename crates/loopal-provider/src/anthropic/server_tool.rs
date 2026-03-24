use serde_json::{Value, json};

/// Client-side tool name that gets replaced with server-side declaration.
pub const WEB_SEARCH_TOOL_NAME: &str = "WebSearch";

/// Anthropic SSE block type for server-side tool invocations.
pub const SERVER_TOOL_USE_TYPE: &str = "server_tool_use";

/// Build the server-side web search tool declaration for the Anthropic API.
///
/// Claude 4.x models use `web_search_20260209` (with dynamic filtering),
/// older models fall back to `web_search_20250305`.
pub fn web_search_tool_definition(model: &str) -> Value {
    let tool_type = if is_claude_4x(model) {
        "web_search_20260209"
    } else {
        "web_search_20250305"
    };
    json!({
        "type": tool_type,
        "name": "web_search",
        "max_uses": 5
    })
}

/// Check if a model belongs to the Claude 4.x family (supports web_search_20260209).
///
/// Matches segments: "4" (claude-sonnet-4-20250514) or "4.6" (claude-opus-4.6).
/// Does NOT match: "40b", "4k" (no dot or exact match).
/// Known models: claude-sonnet-4-*, claude-opus-4-*, claude-sonnet-4.6, claude-opus-4.6.
fn is_claude_4x(model: &str) -> bool {
    if !model.starts_with("claude-") {
        return false;
    }
    // Split on '-' and check for a segment that starts with '4'
    // e.g. "claude-sonnet-4-20250514" → segments ["claude","sonnet","4","20250514"]
    model
        .split('-')
        .any(|seg| seg == "4" || seg.starts_with("4."))
}
