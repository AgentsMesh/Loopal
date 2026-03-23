use std::sync::Arc;

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::PermissionLevel;
use loopal_tool_api::{Tool, ToolContext, ToolResult};

use super::agent::extract_shared;
use crate::shared::AgentShared;

/// Slack-like channel tool — subscribe/unsubscribe/publish/read/list.
///
/// A single tool dispatching on the `operation` parameter. Channels are
/// pull-only: `publish` stores messages in history, subscribers read via
/// `Channel.read`. Direct messages (SendMessage) remain push-based.
pub struct ChannelTool;

#[async_trait]
impl Tool for ChannelTool {
    fn name(&self) -> &str {
        "Channel"
    }

    fn description(&self) -> &str {
        "Named pub/sub channels for group communication (pull-only). Operations: \
         subscribe, unsubscribe, publish, read, list. \
         Published messages are stored in channel history; subscribers read via 'read'."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["subscribe", "unsubscribe", "publish", "read", "list"],
                    "description": "Channel operation to perform"
                },
                "channel": {
                    "type": "string",
                    "description": "Channel name (required for subscribe/unsubscribe/publish/read)"
                },
                "message": {
                    "type": "string",
                    "description": "Message content (required for publish)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max messages to read (default 20, for read only)"
                }
            },
            "required": ["operation"]
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let shared = extract_shared(ctx)?;
        let op = input
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let channel = input.get("channel").and_then(|v| v.as_str());

        match op {
            "subscribe" => op_subscribe(&shared, channel).await,
            "unsubscribe" => op_unsubscribe(&shared, channel).await,
            "publish" => op_publish(&shared, channel, &input).await,
            "read" => op_read(&shared, channel, &input).await,
            "list" => op_list(&shared).await,
            _ => Ok(ToolResult::error(format!("Unknown operation: '{op}'"))),
        }
    }
}

async fn op_subscribe(
    shared: &Arc<AgentShared>,
    channel: Option<&str>,
) -> Result<ToolResult, LoopalError> {
    let ch = require_channel(channel)?;
    shared.router.subscribe(ch, &shared.agent_name).await;
    Ok(ToolResult::success(format!("Subscribed to #{ch}")))
}

async fn op_unsubscribe(
    shared: &Arc<AgentShared>,
    channel: Option<&str>,
) -> Result<ToolResult, LoopalError> {
    let ch = require_channel(channel)?;
    shared.router.unsubscribe(ch, &shared.agent_name).await;
    Ok(ToolResult::success(format!("Unsubscribed from #{ch}")))
}

async fn op_publish(
    shared: &Arc<AgentShared>,
    channel: Option<&str>,
    input: &serde_json::Value,
) -> Result<ToolResult, LoopalError> {
    let ch = require_channel(channel)?;
    let message = input
        .get("message")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                "missing 'message'".into(),
            ))
        })?;

    let subscribers = shared.router.publish(ch, &shared.agent_name, message).await;

    Ok(ToolResult::success(format!(
        "Published to #{ch} — {} subscriber(s) can read via Channel.read",
        subscribers.len(),
    )))
}

async fn op_read(
    shared: &Arc<AgentShared>,
    channel: Option<&str>,
    input: &serde_json::Value,
) -> Result<ToolResult, LoopalError> {
    let ch = require_channel(channel)?;
    let limit = input.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

    let messages = shared.router.read_channel(ch, 0).await;
    let items: Vec<serde_json::Value> = messages
        .iter()
        .rev()
        .take(limit)
        .rev()
        .map(|m| {
            serde_json::json!({
                "from": m.from,
                "content": m.content,
                "timestamp": m.timestamp.to_rfc3339(),
            })
        })
        .collect();

    Ok(ToolResult::success(
        serde_json::to_string_pretty(&items).unwrap_or_default(),
    ))
}

async fn op_list(shared: &Arc<AgentShared>) -> Result<ToolResult, LoopalError> {
    let channels = shared.router.list_channels().await;
    if channels.is_empty() {
        return Ok(ToolResult::success("No channels exist yet."));
    }
    let listing: Vec<String> = channels.iter().map(|c| format!("#{c}")).collect();
    Ok(ToolResult::success(listing.join(", ")))
}

fn require_channel(ch: Option<&str>) -> Result<&str, LoopalError> {
    ch.ok_or_else(|| {
        LoopalError::Tool(loopal_error::ToolError::InvalidInput(
            "missing 'channel'".into(),
        ))
    })
}
