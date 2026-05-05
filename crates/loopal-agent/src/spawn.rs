//! Sub-agent spawning via Hub — all process management delegated to Hub.

use std::path::PathBuf;
use std::sync::Arc;

use loopal_ipc::protocol::methods;
use serde_json::json;

use crate::shared::AgentShared;

/// Where the sub-agent should be spawned — controls which fields cross IPC.
///
/// The two variants exist because in-hub and cross-hub spawns have
/// fundamentally different trust / filesystem-view semantics:
/// - **InHub**: child runs on the same Hub as the parent; cwd / env /
///   parent conversation context (fork_context) can all be inherited
///   safely because they share a filesystem.
/// - **CrossHub** (same-machine cross-hub or remote, treated identically):
///   child runs under a different Hub; the filesystem is NOT shared.
///   `cwd` / `fork_context` / session `resume` MUST NOT cross the boundary —
///   the receiver Hub uses its own `default_cwd`.
pub enum SpawnTarget {
    InHub {
        /// Override the working directory (e.g. for worktree isolation).
        cwd_override: Option<PathBuf>,
        /// Compressed parent conversation injected as initial child context.
        fork_context: Option<Vec<loopal_message::Message>>,
    },
    CrossHub {
        /// Target hub identifier (e.g. "hub-b") registered on MetaHub.
        hub_id: String,
    },
}

/// Parameters for spawning a new sub-agent.
pub struct SpawnParams {
    pub name: String,
    pub prompt: String,
    pub model: Option<String>,
    /// Permission mode hint propagated from parent. The receiver Hub's
    /// permission policy is the enforcement point — for cross-hub spawns
    /// this is an advisory signal, not a command.
    pub permission_mode: Option<String>,
    /// Agent type for fragment selection (e.g. "explore", "plan").
    pub agent_type: Option<String>,
    /// Nesting depth of the child agent (parent depth + 1).
    pub depth: u32,
    /// Reflects the parent's effective `settings.sandbox.policy == Disabled`.
    /// Behavior flag — not filesystem-coupled, so it crosses hub boundaries
    /// safely (unlike `cwd` / `fork_context`).
    pub no_sandbox: bool,
    /// In-hub vs cross-hub semantics.
    pub target: SpawnTarget,
}

/// Result returned from Hub after spawning.
pub struct SpawnResult {
    pub agent_id: String,
    pub name: String,
}

/// Build the IPC payload sent on `hub/spawn_agent`. Pure function — extracted
/// for unit testing the InHub / CrossHub field selection.
pub fn build_spawn_request(
    params: &SpawnParams,
    parent_cwd: &std::path::Path,
) -> serde_json::Value {
    let mut request = json!({
        "name": params.name,
        "model": params.model,
        "prompt": params.prompt,
        "permission_mode": params.permission_mode,
        "agent_type": params.agent_type,
        "depth": params.depth,
        "no_sandbox": params.no_sandbox,
    });

    match &params.target {
        SpawnTarget::InHub {
            cwd_override,
            fork_context,
        } => {
            let cwd = cwd_override
                .as_deref()
                .unwrap_or(parent_cwd)
                .to_string_lossy()
                .to_string();
            request["cwd"] = json!(cwd);
            if let Some(fc) = fork_context
                && let Ok(val) = serde_json::to_value(fc)
            {
                request["fork_context"] = val;
            }
        }
        SpawnTarget::CrossHub { hub_id } => {
            // CrossHub: no cwd, no fork_context, no resume. Receiver Hub's
            // own default_cwd is used; filesystem is not shared.
            request["target_hub"] = json!(hub_id);
        }
    }

    request
}

/// Request Hub to spawn a sub-agent. Hub handles fork, stdio, and registration.
pub async fn spawn_agent(
    shared: &Arc<AgentShared>,
    params: SpawnParams,
) -> Result<SpawnResult, String> {
    let request = build_spawn_request(&params, &shared.cwd);

    tracing::info!(agent = %params.name, "sending hub/spawn_agent request");
    let response = shared
        .hub_connection
        .send_request(methods::HUB_SPAWN_AGENT.name, request)
        .await
        .map_err(|e| format!("hub/spawn_agent failed: {e}"))?;
    tracing::info!(agent = %params.name, "hub/spawn_agent response received");

    let agent_id = response["agent_id"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    Ok(SpawnResult {
        agent_id,
        name: params.name,
    })
}

/// Wait for a spawned agent to finish. Returns its final output.
pub async fn wait_agent(shared: &Arc<AgentShared>, name: &str) -> Result<String, String> {
    let request = json!({"name": name});
    tracing::info!(agent = %name, "sending hub/wait_agent request");
    let response = shared
        .hub_connection
        .send_request(methods::HUB_WAIT_AGENT.name, request)
        .await
        .map_err(|e| format!("hub/wait_agent failed: {e}"))?;
    tracing::info!(agent = %name, "hub/wait_agent response received");

    Ok(response["output"]
        .as_str()
        .unwrap_or("(no output)")
        .to_string())
}
