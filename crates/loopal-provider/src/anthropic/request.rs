use loopal_provider_api::ChatParams;
use serde_json::{Value, json};

use super::AnthropicProvider;
use super::server_tool;

impl AnthropicProvider {
    pub fn build_tools(&self, params: &ChatParams) -> Vec<Value> {
        let mut tools: Vec<Value> = Vec::new();
        let mut last_client_idx: Option<usize> = None;

        for tool in &params.tools {
            if tool.name == server_tool::WEB_SEARCH_TOOL_NAME {
                // Replace client-side WebSearch with server-side declaration
                tools.push(server_tool::web_search_tool_definition(&params.model));
            } else {
                last_client_idx = Some(tools.len());
                tools.push(json!({
                    "name": tool.name,
                    "description": tool.description,
                    "input_schema": tool.input_schema
                }));
            }
        }

        // Place cache_control on the last client-side tool (not server tools)
        if let Some(idx) = last_client_idx {
            tools[idx]["cache_control"] = json!({"type": "ephemeral"});
        }

        tools
    }
}
