//! Sub-agent spawning via Hub — all process management delegated to Hub.

use std::path::PathBuf;
use std::sync::Arc;

use loopal_ipc::protocol::methods;
use serde_json::json;

use crate::shared::AgentShared;

/// Parameters for spawning a new sub-agent.
pub struct SpawnParams {
    pub name: String,
    pub prompt: String,
    pub model: Option<String>,
    /// Override the working directory (e.g. for worktree isolation).
    pub cwd_override: Option<PathBuf>,
}

/// Result returned from Hub after spawning.
pub struct SpawnResult {
    pub agent_id: String,
    pub name: String,
}

/// Request Hub to spawn a sub-agent. Hub handles fork, stdio, and registration.
pub async fn spawn_agent(
    shared: &Arc<AgentShared>,
    params: SpawnParams,
) -> Result<SpawnResult, String> {
    let cwd = params
        .cwd_override
        .as_deref()
        .unwrap_or(&shared.cwd)
        .to_string_lossy()
        .to_string();

    let request = json!({
        "name": params.name,
        "cwd": cwd,
        "model": params.model,
        "prompt": params.prompt,
    });

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
