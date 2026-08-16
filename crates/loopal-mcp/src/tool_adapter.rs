use std::sync::Arc;

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolDefinition, ToolResult};
use rmcp::model::CallToolResult;
use serde_json::Value;
use tracing::Instrument;

use crate::provider::McpProvider;
use crate::tool_result_text::{block_to_text, call_result_to_response};

pub const MCP_SECRET_ARG_REJECTION: &str =
    "MCP tool arguments cannot contain secret placeholders; configure the MCP server instead";

pub fn contains_secret_placeholder(value: &Value) -> bool {
    match value {
        Value::String(value) => {
            loopal_secret_client::AUTHOR_RE.is_match(value)
                || loopal_secret_client::WIRE_RE.is_match(value)
        }
        Value::Array(values) => values.iter().any(contains_secret_placeholder),
        Value::Object(values) => values
            .iter()
            .any(|(key, value)| contains_secret_text(key) || contains_secret_placeholder(value)),
        _ => false,
    }
}

pub struct McpToolAdapter {
    definition: ToolDefinition,
    server_name: String,
    provider: Arc<dyn McpProvider>,
}

impl McpToolAdapter {
    pub fn new(
        definition: ToolDefinition,
        server_name: String,
        provider: Arc<dyn McpProvider>,
    ) -> Self {
        Self {
            definition,
            server_name,
            provider,
        }
    }
}

#[async_trait]
impl Tool for McpToolAdapter {
    fn name(&self) -> &str {
        &self.definition.name
    }

    fn description(&self) -> &str {
        &self.definition.description
    }

    fn parameters_schema(&self) -> Value {
        self.definition.input_schema.clone()
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    fn precheck(&self, input: &Value) -> Option<String> {
        contains_secret_placeholder(input).then(|| MCP_SECRET_ARG_REJECTION.into())
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let mcp_span =
            tracing::info_span!("mcp_tool_call", mcp.tool = self.definition.name.as_str());
        async {
            // reason: tool execution runs after the agent loop is up and
            // the reverse channel is draining — default IPC budget is fine.
            let budget = loopal_ipc::HUB_RPC_BUDGET;
            let result = self
                .provider
                .call_tool(&self.server_name, &self.definition.name, &input, budget)
                .await
                .map_err(LoopalError::Mcp)?;

            Ok(convert_tool_result(&result))
        }
        .instrument(mcp_span)
        .await
    }
}

fn convert_tool_result(result: &CallToolResult) -> ToolResult {
    let response = call_result_to_response(result);
    let parts: Vec<String> = response.content.iter().map(block_to_text).collect();

    ToolResult {
        content: parts.join("\n"),
        images: Vec::new(),
        is_error: response.is_error,
        metadata: None,
    }
}

fn contains_secret_text(value: &str) -> bool {
    loopal_secret_client::AUTHOR_RE.is_match(value) || loopal_secret_client::WIRE_RE.is_match(value)
}

#[cfg(test)]
#[path = "tool_adapter_tests.rs"]
mod tests;
