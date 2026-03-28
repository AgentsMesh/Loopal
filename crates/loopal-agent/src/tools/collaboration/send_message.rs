//! SendMessage tool — point-to-point message routing via Hub.

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_ipc::protocol::methods;
use loopal_protocol::{Envelope, MessageSource};
use loopal_tool_api::PermissionLevel;
use loopal_tool_api::{Tool, ToolContext, ToolResult};

use super::agent::extract_shared;

/// Send a message to a named agent via Hub routing.
pub struct SendMessageTool;

#[async_trait]
impl Tool for SendMessageTool {
    fn name(&self) -> &str { "SendMessage" }

    fn description(&self) -> &str {
        "Send a message to another agent by name. The message is routed through \
         the Hub. Only works if the target agent is currently running."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "required": ["to", "message"],
            "properties": {
                "to": { "type": "string", "description": "Target agent name" },
                "message": { "type": "string", "description": "Message content" },
                "summary": { "type": "string", "description": "Short preview for UI" }
            }
        })
    }

    fn permission(&self) -> PermissionLevel { PermissionLevel::ReadOnly }

    async fn execute(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let shared = extract_shared(ctx)?;

        let target = input["to"].as_str().ok_or_else(|| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                "missing 'to'".into(),
            ))
        })?;
        let content = input["message"].as_str().unwrap_or("");

        let envelope = Envelope::new(
            MessageSource::Agent(shared.agent_name.clone()),
            target,
            content,
        );
        let params = serde_json::to_value(&envelope).map_err(|e| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(e.to_string()))
        })?;

        match shared
            .hub_connection
            .send_request(methods::HUB_ROUTE.name, params)
            .await
        {
            Ok(_) => Ok(ToolResult::success(format!("Message sent to '{target}'"))),
            Err(e) => Ok(ToolResult::error(format!(
                "Failed to send to '{target}': {e}"
            ))),
        }
    }
}
