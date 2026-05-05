//! `Agent` tool — spawn action: parses tool input, builds `SpawnParams`, and
//! drives the foreground/background completion + worktree cleanup flow.

use std::sync::Arc;

use loopal_error::LoopalError;
use loopal_tool_api::ToolResult;

use super::agent_fork::{build_fork_context, spawn_bg_cleanup};
use super::shared_extract::{create_agent_worktree, require_str};
use super::spawn_decision::{build_spawn_target, worktree_allowed};
use crate::config::load_agent_configs;
use crate::shared::AgentShared;
use crate::spawn::{SpawnParams, spawn_agent, wait_agent};

pub(super) async fn action_spawn(
    shared: Arc<AgentShared>,
    input: &serde_json::Value,
    memory_channel: Option<&dyn loopal_tool_api::MemoryChannel>,
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
    let target_hub = input
        .get("target_hub")
        .and_then(|v| v.as_str())
        .map(String::from);

    let mut config = subagent_type
        .and_then(|t| load_agent_configs(&shared.cwd).remove(t))
        .unwrap_or_default();
    if let Some(ref m) = model_override {
        config.model = Some(m.clone());
    }
    // Worktree isolation only applies to in-hub spawns (cross-hub child runs
    // on a different filesystem; the worktree path would not exist there).
    let wt = if worktree_allowed(&target_hub, isolation) {
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
    let no_sandbox = shared.no_sandbox();
    let target = build_spawn_target(target_hub, cwd_override, build_fork_context(&shared));
    let result = spawn_agent(
        &shared,
        SpawnParams {
            name: name.clone(),
            prompt: prompt.to_string(),
            model: Some(model),
            permission_mode: Some(perm_mode.to_string()),
            agent_type: subagent_type.map(String::from),
            depth: shared.depth + 1,
            no_sandbox,
            target,
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
                    Ok(text) => {
                        if let Some(ch) = memory_channel {
                            for suggestion in
                                loopal_memory::extraction::extract_memory_suggestions(&text)
                            {
                                let _ = ch.try_send(suggestion);
                            }
                        }
                        Ok(ToolResult::success(text))
                    }
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
