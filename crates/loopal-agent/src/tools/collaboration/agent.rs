use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_ipc::protocol::methods;
use loopal_tool_api::PermissionLevel;
use loopal_tool_api::{Tool, ToolContext, ToolResult};
use serde_json::json;
use std::sync::Arc;

use super::agent_spawn::action_spawn;
use super::shared_extract::{extract_shared, require_str};
use crate::shared::AgentShared;
use crate::spawn::wait_agent;

pub struct AgentTool;

#[async_trait]
impl Tool for AgentTool {
    fn name(&self) -> &str {
        "Agent"
    }
    fn description(&self) -> &str {
        "Spawn a sub-agent. Blocks until complete. Multiple calls run in parallel."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["spawn", "result", "status"] },
                "prompt": { "type": "string" },
                "name": { "type": "string" },
                "subagent_type": { "type": "string" },
                "model": { "type": "string" },
                "target_hub": {
                    "type": "string",
                    "description": "Spawn on a remote hub in the cluster (e.g. 'hub-b'). Requires MetaHub connection."
                },
                "run_in_background": {
                    "type": "boolean",
                    "description": "Default false (foreground). Only set true when you have independent work to do while the agent runs."
                },
                "isolation": { "type": "string", "enum": ["worktree"] }
            },
            "required": ["prompt"]
        })
    }
    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Supervised
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let shared = extract_shared(ctx)?;
        let memory_channel = ctx.memory_channel.clone();
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("spawn");

        match action {
            "spawn" => action_spawn(shared, &input, memory_channel.as_deref()).await,
            "result" => action_result(shared, &input).await,
            "status" => action_status(shared, &input).await,
            other => Ok(ToolResult::error(format!("Unknown action: '{other}'"))),
        }
    }
}

async fn action_result(
    shared: Arc<AgentShared>,
    input: &serde_json::Value,
) -> Result<ToolResult, LoopalError> {
    let name = require_str(input, "name")?;
    match wait_agent(&shared, name).await {
        Ok(output) => Ok(ToolResult::success(output)),
        Err(e) => Ok(ToolResult::error(e)),
    }
}

async fn action_status(
    shared: Arc<AgentShared>,
    input: &serde_json::Value,
) -> Result<ToolResult, LoopalError> {
    let name = require_str(input, "name")?;
    match shared
        .hub_connection
        .send_request(methods::HUB_AGENT_INFO.name, json!({"name": name}))
        .await
    {
        Ok(info) => Ok(ToolResult::success(
            serde_json::to_string_pretty(&info).unwrap_or_default(),
        )),
        Err(e) => Ok(ToolResult::error(format!("Agent '{name}': {e}"))),
    }
}
