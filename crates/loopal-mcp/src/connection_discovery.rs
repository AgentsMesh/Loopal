use loopal_tool_api::ToolDefinition;
use serde_json::Value;

use crate::connection::McpConnection;
use crate::result_sanitizer::CallResultSanitizer;
use crate::types::{CapabilitySummary, McpPrompt, McpResource};

pub(super) async fn discover_capabilities(
    connection: &mut McpConnection,
    sanitizer: &CallResultSanitizer,
) {
    let caps = connection
        .client()
        .map(extract_capabilities)
        .unwrap_or_default();
    if caps.tools {
        let result = match connection.client() {
            Some(client) => client.list_tools().await,
            None => return,
        };
        match result {
            Ok(result) => {
                connection.cached_tools = result
                    .tools
                    .iter()
                    .map(|tool| {
                        let mut input_schema = Value::Object((*tool.input_schema).clone());
                        sanitizer.sanitize_json(&mut input_schema);
                        ToolDefinition {
                            name: sanitizer.sanitize_text(&tool.name),
                            description: tool
                                .description
                                .as_deref()
                                .map(|value| sanitizer.sanitize_text(value))
                                .unwrap_or_default(),
                            input_schema,
                        }
                    })
                    .collect();
            }
            Err(_) => connection.errors.push("tools/list failed".into()),
        }
    }
    if caps.resources {
        let result = match connection.client() {
            Some(client) => client.list_resources().await,
            None => return,
        };
        match result {
            Ok(result) => {
                connection.cached_resources = result
                    .resources
                    .iter()
                    .map(|resource| McpResource {
                        uri: sanitizer.sanitize_text(&resource.uri),
                        name: sanitizer.sanitize_text(&resource.name),
                        description: resource
                            .description
                            .as_deref()
                            .map(|value| sanitizer.sanitize_text(value)),
                        mime_type: resource
                            .mime_type
                            .as_deref()
                            .map(|value| sanitizer.sanitize_text(value)),
                    })
                    .collect();
            }
            Err(_) => connection.errors.push("resources/list failed".into()),
        }
    }
    if caps.prompts {
        let result = match connection.client() {
            Some(client) => client.list_prompts().await,
            None => return,
        };
        match result {
            Ok(result) => {
                connection.cached_prompts = result
                    .prompts
                    .iter()
                    .map(|prompt| McpPrompt {
                        name: sanitizer.sanitize_text(&prompt.name),
                        description: prompt
                            .description
                            .as_deref()
                            .map(|value| sanitizer.sanitize_text(value)),
                    })
                    .collect();
            }
            Err(_) => connection.errors.push("prompts/list failed".into()),
        }
    }
}

fn extract_capabilities(client: &crate::client::McpClient) -> CapabilitySummary {
    let Some(info) = client.peer_info() else {
        return CapabilitySummary::default();
    };
    CapabilitySummary {
        tools: info.capabilities.tools.is_some(),
        resources: info.capabilities.resources.is_some(),
        prompts: info.capabilities.prompts.is_some(),
    }
}

#[cfg(test)]
#[path = "connection_discovery_tests.rs"]
mod tests;
