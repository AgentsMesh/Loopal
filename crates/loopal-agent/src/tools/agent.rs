use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::PermissionLevel;
use loopal_tool_api::{Tool, ToolContext, ToolResult};

use crate::config::load_agent_configs;
use crate::shared::AgentShared;
use crate::spawn::{SpawnParams, spawn_agent};

/// Tool that spawns a new sub-agent to work on a task.
pub struct AgentTool;

#[async_trait]
impl Tool for AgentTool {
    fn name(&self) -> &str {
        "Agent"
    }

    fn description(&self) -> &str {
        "Spawn a sub-agent to handle a task autonomously. \
         The sub-agent runs in the background and can use tools independently."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string", "description": "The task for the sub-agent" },
                "description": { "type": "string", "description": "A short (3-5 word) summary" },
                "name": { "type": "string", "description": "Agent name (auto-generated if omitted)" },
                "subagent_type": { "type": "string", "description": "Agent type from .loopal/agents/" },
                "model": { "type": "string", "description": "LLM model override" },
                "run_in_background": { "type": "boolean", "description": "Run without blocking" },
                "isolation": { "type": "string", "enum": ["worktree"], "description": "Run in isolated git worktree" }
            },
            "required": ["prompt", "description"]
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
        execute_agent(extract_shared(ctx)?, input).await
    }
}

async fn execute_agent(
    shared: Arc<AgentShared>,
    input: serde_json::Value,
) -> Result<ToolResult, LoopalError> {
    let prompt = require_str(&input, "prompt")?;
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
    if shared.registry.lock().await.get(&name).is_some() {
        return Ok(ToolResult::error(format!("Agent '{name}' already exists")));
    }

    let mut config = subagent_type
        .and_then(|t| load_agent_configs(&shared.cwd).remove(t))
        .unwrap_or_default();
    if let Some(ref m) = model_override {
        config.model = Some(m.clone());
    }

    // Worktree isolation (name includes UUID to avoid collisions across re-spawns)
    let wt = if isolation == Some("worktree") {
        let uid = &uuid::Uuid::new_v4().to_string()[..8];
        Some(create_agent_worktree(&shared.cwd, &name, uid)?)
    } else {
        None
    };

    let cwd_override = wt.as_ref().map(|(info, _)| info.path.clone());
    let result = spawn_agent(
        &shared,
        SpawnParams {
            name: name.clone(),
            prompt: prompt.to_string(),
            agent_config: config,
            parent_model: shared.kernel.settings().model.clone(),
            parent_cancel_token: None,
            cwd_override,
            worktree: wt.clone(),
        },
    )
    .await;

    match result {
        Ok(sr) => {
            shared.registry.lock().await.register(sr.handle);
            handle_spawn_result(sr.agent_id, sr.result_rx, name, background, &wt).await
        }
        Err(e) => {
            // Worktree cleanup is normally handled by spawn's JoinHandle, but if
            // spawn itself failed, the JoinHandle was never created — clean up here.
            if let Some((info, root)) = wt {
                loopal_git::cleanup_if_clean(&root, &info);
            }
            Ok(ToolResult::error(format!("Failed to spawn agent: {e}")))
        }
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

/// Handle the result of a spawned sub-agent.
///
/// Worktree cleanup is handled by the tracked JoinHandle in `spawn.rs`,
/// not here — avoiding fire-and-forget tasks that could be silently cancelled.
async fn handle_spawn_result(
    agent_id: String,
    result_rx: tokio::sync::oneshot::Receiver<Result<String, String>>,
    name: String,
    background: bool,
    wt: &Option<(loopal_git::WorktreeInfo, PathBuf)>,
) -> Result<ToolResult, LoopalError> {
    if background {
        let wt_suffix = wt
            .as_ref()
            .map(|(info, _)| format!(" (worktree: {})", info.path.display()))
            .unwrap_or_default();
        return Ok(ToolResult::success(format!(
            "Agent '{name}' spawned in background{wt_suffix}.\nagentId: {agent_id}",
        )));
    }
    match result_rx.await {
        Ok(Ok(out)) => Ok(ToolResult::success(out)),
        Ok(Err(err)) => Ok(ToolResult::error(err)),
        Err(_) => Ok(ToolResult::error("sub-agent terminated unexpectedly")),
    }
}

/// Extract `AgentShared` from `ToolContext.shared`.
pub(crate) fn extract_shared(ctx: &ToolContext) -> Result<Arc<AgentShared>, LoopalError> {
    ctx.shared
        .as_ref()
        .and_then(|s| s.downcast_ref::<Arc<AgentShared>>())
        .cloned()
        .ok_or_else(|| LoopalError::Other("AgentShared not available in ToolContext".into()))
}

fn require_str<'a>(input: &'a serde_json::Value, key: &str) -> Result<&'a str, LoopalError> {
    input.get(key).and_then(|v| v.as_str()).ok_or_else(|| {
        LoopalError::Tool(loopal_error::ToolError::InvalidInput(format!(
            "missing '{key}'"
        )))
    })
}
