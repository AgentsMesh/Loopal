//! Agent tool — self-contained multi-agent collaboration via Hub.
//!
//! Actions:
//! - spawn: create a new sub-agent (foreground blocks, background returns name)
//! - result: wait for a background agent to finish and return its output
//! - status: non-blocking query of agent lifecycle and topology

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_ipc::protocol::methods;
use loopal_tool_api::PermissionLevel;
use loopal_tool_api::{Tool, ToolContext, ToolResult};
use serde_json::json;

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
        "Multi-agent collaboration: spawn sub-agents, get results, query status"
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
                "run_in_background": { "type": "boolean" },
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

/// Spawn a new sub-agent. Foreground blocks until done; background returns name.
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

    let result = spawn_agent(
        &shared,
        SpawnParams {
            name: name.clone(),
            prompt: prompt.to_string(),
            model: Some(model),
            cwd_override,
        },
    )
    .await;

    match result {
        Ok(sr) => {
            if background {
                // Worktree cleanup in background
                if let Some((info, root)) = wt {
                    let s = shared.clone();
                    let n = name.clone();
                    tokio::spawn(async move {
                        let _ = wait_agent(&s, &n).await;
                        loopal_git::cleanup_if_clean(&root, &info);
                    });
                }
                Ok(ToolResult::success(format!(
                    "Agent '{name}' spawned in background.\nagentId: {}",
                    sr.agent_id,
                )))
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

/// Wait for a background agent to finish and return its output.
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

/// Non-blocking query of agent status via Hub topology.
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

fn create_agent_worktree(
    cwd: &Path,
    agent_name: &str,
    uid: &str,
) -> Result<(loopal_git::WorktreeInfo, PathBuf), LoopalError> {
    let root = loopal_git::repo_root(cwd)
        .ok_or_else(|| LoopalError::Other("Not a git repository".into()))?;
    let wt_name = format!("agent-{agent_name}-{uid}");
    let info = loopal_git::create_worktree(&root, &wt_name)
        .map_err(|e| LoopalError::Other(format!("worktree: {e}")))?;
    Ok((info, root))
}

/// Extract `AgentShared` from `ToolContext.shared`.
pub(crate) fn extract_shared(ctx: &ToolContext) -> Result<Arc<AgentShared>, LoopalError> {
    ctx.shared
        .as_ref()
        .and_then(|s| s.downcast_ref::<Arc<AgentShared>>())
        .cloned()
        .ok_or_else(|| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                "AgentShared not available".into(),
            ))
        })
}

fn require_str<'a>(input: &'a serde_json::Value, field: &str) -> Result<&'a str, LoopalError> {
    input.get(field).and_then(|v| v.as_str()).ok_or_else(|| {
        LoopalError::Tool(loopal_error::ToolError::InvalidInput(format!(
            "missing '{field}'"
        )))
    })
}
