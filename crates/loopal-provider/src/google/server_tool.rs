use serde_json::{Value, json};

/// Client-side tool name that gets replaced with server-side declaration.
pub const WEB_SEARCH_TOOL_NAME: &str = "WebSearch";

/// Build the Google Search Grounding tool declaration for the Gemini API.
pub fn google_search_tool_definition() -> Value {
    json!({"googleSearch": {}})
}
