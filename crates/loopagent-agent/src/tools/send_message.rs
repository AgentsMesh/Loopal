use async_trait::async_trait;
use loopagent_types::envelope::{Envelope, MessageSource};
use loopagent_types::error::LoopAgentError;
use loopagent_types::permission::PermissionLevel;
use loopagent_types::tool::{Tool, ToolContext, ToolResult};

use super::agent::extract_shared;

/// Tool for inter-agent messaging: point-to-point, broadcast, and shutdown.
pub struct SendMessageTool;

#[async_trait]
impl Tool for SendMessageTool {
    fn name(&self) -> &str { "SendMessage" }

    fn description(&self) -> &str {
        "Send a message to another agent. Supports types: \
         'message' (point-to-point), 'broadcast' (all agents), \
         'shutdown_request' (ask agent to stop)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "type": {
                    "type": "string",
                    "enum": ["message", "broadcast", "shutdown_request"],
                    "description": "Message type"
                },
                "recipient": {
                    "type": "string",
                    "description": "Target agent name (required for message/shutdown)"
                },
                "content": {
                    "type": "string",
                    "description": "Message content"
                }
            },
            "required": ["type", "content"]
        })
    }

    fn permission(&self) -> PermissionLevel { PermissionLevel::ReadOnly }

    async fn execute(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopAgentError> {
        let shared = extract_shared(ctx)?;

        let msg_type = input.get("type").and_then(|v| v.as_str()).unwrap_or("message");
        let content = input.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let recipient = input.get("recipient").and_then(|v| v.as_str());

        match msg_type {
            "message" => {
                let target = require_recipient(recipient)?;
                let envelope = Envelope::new(
                    MessageSource::Agent(shared.agent_name.clone()),
                    target,
                    content,
                );
                match shared.router.route(envelope).await {
                    Ok(()) => Ok(ToolResult::success(format!("Message sent to '{target}'"))),
                    Err(e) => Ok(ToolResult::error(e)),
                }
            }
            "broadcast" => {
                let envelope = Envelope::new(
                    MessageSource::Agent(shared.agent_name.clone()),
                    "", // target filled per-recipient by broadcast()
                    content,
                );
                match shared.router.broadcast(envelope, Some(&shared.agent_name)).await {
                    Ok(delivered) => Ok(ToolResult::success(format!(
                        "Broadcast sent to {} agents", delivered.len()
                    ))),
                    Err(e) => Ok(ToolResult::error(e)),
                }
            }
            "shutdown_request" => {
                let target = require_recipient(recipient)?;
                let registry = shared.registry.lock().await;
                if let Some(handle) = registry.get(target) {
                    handle.cancel_token.cancel();
                    Ok(ToolResult::success(format!("Shutdown request sent to '{target}'")))
                } else {
                    Ok(ToolResult::error(format!("Agent '{target}' not found")))
                }
            }
            other => Ok(ToolResult::error(format!("Unknown message type: '{other}'"))),
        }
    }
}

fn require_recipient(r: Option<&str>) -> Result<&str, LoopAgentError> {
    r.ok_or_else(|| LoopAgentError::Tool(
        loopagent_types::error::ToolError::InvalidInput("missing 'recipient'".into()),
    ))
}
