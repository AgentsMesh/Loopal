use serde_json::{Value, json};

/// Client-side tool name that gets replaced with server-side declaration.
pub const WEB_SEARCH_TOOL_NAME: &str = "WebSearch";

/// Build the web_search tool declaration for the OpenAI Responses API.
pub fn web_search_tool_definition() -> Value {
    json!({
        "type": "web_search",
        "search_context_size": "medium"
    })
}
