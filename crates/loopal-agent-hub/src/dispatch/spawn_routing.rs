//! Entry handlers for `hub/spawn_agent` (in-hub) and
//! `hub/spawn_remote_agent` (cross-hub-receiver).
//!
//! In-hub spawn assumes shared filesystem: caller may pass `cwd` and
//! `fork_context`. Cross-hub spawn is forbidden from carrying those —
//! the receiver uses its own `Hub.default_cwd` and rejects fork_context
//! / resume. Cross-hub forwarding lives in `cross_hub_forward.rs`.

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::Mutex;
use tracing::info;

use crate::hub::Hub;

/// In-hub spawn entry point. If `target_hub` is set, forward to MetaHub
/// after rejecting any filesystem-coupled fields (cwd / fork_context /
/// resume). Otherwise spawn locally.
pub async fn handle_spawn_agent(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    from_agent: &str,
) -> Result<Value, String> {
    if let Some(v) = params.get("target_hub") {
        let target = v
            .as_str()
            .ok_or_else(|| format!("'target_hub' must be a string, got: {v}"))?;
        // Mirror the agent-name check in cross_hub_forward::preflight: a
        // hub identifier with '/' would be ambiguous with QualifiedAddress
        // multi-hop encoding (`hub-c/hub-d/agent`) — reject up front.
        if target.contains('/') {
            return Err(format!(
                "'target_hub' cannot contain '/' (cross-hub address encoding), got: {target}"
            ));
        }
        return super::cross_hub_forward::forward_cross_hub_spawn(hub, params, from_agent).await;
    }
    spawn_local(hub, params, from_agent).await
}

async fn spawn_local(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    from_agent: &str,
) -> Result<Value, String> {
    let name = params["name"]
        .as_str()
        .ok_or("missing 'name' field")?
        .to_string();
    let cwd = params["cwd"].as_str().unwrap_or(".").to_string();
    let model = params["model"].as_str().map(String::from);
    let prompt = params["prompt"].as_str().map(String::from);
    let permission_mode = params["permission_mode"].as_str().map(String::from);
    let agent_type = params["agent_type"].as_str().map(String::from);
    let depth = params["depth"].as_u64().map(|v| v as u32);
    let fork_context = params.get("fork_context").cloned();
    let parent = params["parent"]
        .as_str()
        .map(String::from)
        .or_else(|| Some(from_agent.to_string()));

    info!(agent = %name, parent = ?parent, "handle_spawn_agent local start");
    spawn_via_manager(
        hub.clone(),
        name,
        cwd,
        model,
        prompt,
        parent,
        permission_mode,
        agent_type,
        depth,
        fork_context,
    )
    .await
}

/// Cross-hub spawn target: MetaHub forwards `meta/spawn` here as
/// `hub/spawn_remote_agent`. Caller has no shared filesystem, so
/// `cwd` / `fork_context` / `resume` are forbidden — receiver uses its
/// own `Hub.default_cwd`.
pub async fn handle_spawn_remote_agent(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    from_agent: &str,
) -> Result<Value, String> {
    let default_cwd = hub.lock().await.default_cwd.clone();
    let args =
        super::spawn_prepare::prepare_remote_spawn_args(&params, from_agent, &default_cwd)?;
    info!(agent = %args.name, parent = ?args.parent, "handle_spawn_remote_agent start");
    spawn_via_manager(
        hub.clone(),
        args.name,
        args.cwd,
        args.model,
        args.prompt,
        args.parent,
        args.permission_mode,
        args.agent_type,
        args.depth,
        None,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn spawn_via_manager(
    hub: Arc<Mutex<Hub>>,
    name: String,
    cwd: String,
    model: Option<String>,
    prompt: Option<String>,
    parent: Option<String>,
    permission_mode: Option<String>,
    agent_type: Option<String>,
    depth: Option<u32>,
    fork_context: Option<Value>,
) -> Result<Value, String> {
    let name_clone = name.clone();
    // Detached on purpose: spawn_and_register may have already forked a
    // child process, and we don't want outer cancellation to leave the
    // child as an orphan. We still .await for the agent_id so the IPC
    // response carries it back to the caller.
    let handle = tokio::spawn(async move {
        crate::spawn_manager::spawn_and_register(
            hub,
            name_clone,
            cwd,
            model,
            prompt,
            parent,
            permission_mode,
            agent_type,
            depth,
            fork_context,
        )
        .await
    });
    let agent_id = handle
        .await
        .map_err(|e| format!("spawn task failed: {e}"))?
        .map_err(|e| format!("spawn failed: {e}"))?;
    info!(agent = %name, %agent_id, "spawn done");
    Ok(json!({"agent_id": agent_id, "name": name}))
}
