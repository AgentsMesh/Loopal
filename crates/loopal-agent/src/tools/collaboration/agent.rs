use std::sync::Arc;

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_ipc::protocol::methods;
use loopal_tool_api::PermissionLevel;
use loopal_tool_api::{Tool, ToolContext, ToolResult};
use serde_json::json;

use super::shared_extract::{create_agent_worktree, extract_shared, require_str};
use crate::config::load_agent_configs;
use crate::shared::AgentShared;
use crate::spawn::{SpawnParams, spawn_agent, wait_agent};

pub struct AgentTool;

#[async_trait]
impl Tool for AgentTool {
    fn name(&self) -> &str {
        "Agent"
    }
    fn description(&self) -> &str {
        "Spawn a sub-agent to handle a task. Blocks until the agent completes \
         and returns its result. Multiple Agent calls in one turn run in parallel."
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
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("spawn");

        match action {
            "spawn" => action_spawn(shared, &input).await,
            "result" => action_result(shared, &input).await,
            "status" => action_status(shared, &input).await,
            other => Ok(ToolResult::error(format!("Unknown action: '{other}'"))),
        }
    }
}

async fn action_spawn(
    shared: Arc<AgentShared>,
    input: &serde_json::Value,
) -> Result<ToolResult, LoopalError> {
    let prompt = require_str(input, "prompt")?;
    let name = input
        .get("name")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| format!("agent-{}", &uuid::Uuid::new_v4().to_string()[..8]));
    let subagent_type = input.get("subagent_type").and_then(|v| v.as_str());
    let model_override = input
        .get("model")
        .and_then(|v| v.as_str())
        .map(String::from);
    let background = input
        .get("run_in_background")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let isolation = input.get("isolation").and_then(|v| v.as_str());
    if shared.depth >= shared.max_depth {
        return Ok(ToolResult::error(format!(
            "Maximum nesting depth ({}) reached",
            shared.max_depth
        )));
    }

    let mut config = subagent_type
        .and_then(|t| load_agent_configs(&shared.cwd).remove(t))
        .unwrap_or_default();
    if let Some(ref m) = model_override {
        config.model = Some(m.clone());
    }
    let wt = if isolation == Some("worktree") {
        let uid = &uuid::Uuid::new_v4().to_string()[..8];
        Some(create_agent_worktree(&shared.cwd, &name, uid)?)
    } else {
        None
    };
    let cwd_override = wt.as_ref().map(|(info, _)| info.path.clone());
    let model = config
        .model
        .unwrap_or_else(|| shared.kernel.settings().model.clone());
    let perm_mode = match shared.kernel.settings().permission_mode {
        loopal_tool_api::PermissionMode::Bypass => "bypass",
        loopal_tool_api::PermissionMode::Supervised => "supervised",
        loopal_tool_api::PermissionMode::Auto => "auto",
    };
    let result = spawn_agent(
        &shared,
        SpawnParams {
            name: name.clone(),
            prompt: prompt.to_string(),
            model: Some(model),
            cwd_override,
            permission_mode: Some(perm_mode.to_string()),
        },
    )
    .await;
    match result {
        Ok(sr) => {
            if background {
                spawn_bg_cleanup(shared.clone(), name.clone(), wt);
                let msg = format!(
                    "Agent '{name}' spawned in background (agentId: {}).\n\
                     Result will be injected into your conversation when it completes.",
                    sr.agent_id,
                );
                Ok(ToolResult::success(msg))
            } else {
                let output = wait_agent(&shared, &name).await;
                if let Some((info, root)) = wt {
                    loopal_git::cleanup_if_clean(&root, &info);
                }
                match output {
                    Ok(text) => Ok(ToolResult::success(text)),
                    Err(e) => Ok(ToolResult::error(e)),
                }
            }
        }
        Err(e) => {
            if let Some((info, root)) = wt {
                loopal_git::cleanup_if_clean(&root, &info);
            }
            Ok(ToolResult::error(format!("Failed to spawn agent: {e}")))
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

fn spawn_bg_cleanup(
    shared: Arc<AgentShared>,
    name: String,
    wt: Option<(loopal_git::WorktreeInfo, std::path::PathBuf)>,
) {
    if let Some((info, root)) = wt {
        tokio::spawn(async move {
            let timeout = std::time::Duration::from_secs(3600);
            match tokio::time::timeout(timeout, wait_agent(&shared, &name)).await {
                Ok(_) => {
                    loopal_git::cleanup_if_clean(&root, &info);
                }
                Err(_) => tracing::warn!(agent = %name, "background agent timed out"),
            }
        });
    }
}
