//! Pure-function preparation step for `hub/spawn_remote_agent`. Extracted
//! from the IPC handler so unit tests can verify field handling — including
//! that the receiver's `default_cwd` is used, not any value the caller sent —
//! without depending on `spawn_manager` (which spawns real subprocesses).
//!
//! TODO(P3): `permission_mode` and `depth` are passed through to the
//! receiving Hub's `apply_start_overrides` / `build_depth_tool_filter` after
//! a minimal clamp (depth >= 1). A malicious cross-hub caller can still
//! influence these — for example, requesting `Bypass` permission_mode, or
//! sending `depth: 1` to keep spawn tools available longer than the local
//! ancestor chain warrants. This is acceptable today because cross-hub
//! Loopal instances are co-located on the same machine + same user. A
//! separate PR must add receiver-side policy arbitration before any
//! cross-network deployment (clamp Bypass → Supervised, configurable
//! per-hub allow-list, depth-floor based on receiver configuration, etc.).

use std::path::Path;

use serde_json::Value;

pub(crate) struct RemoteSpawnArgs {
    pub name: String,
    pub cwd: String,
    pub model: Option<String>,
    pub prompt: Option<String>,
    pub permission_mode: Option<String>,
    pub agent_type: Option<String>,
    pub depth: Option<u32>,
    pub parent: Option<String>,
}

/// Validate a cross-hub spawn payload and assemble args using the receiver
/// Hub's `default_cwd`. Filesystem-coupled fields (`cwd` / `fork_context` /
/// `resume`) are rejected because the caller cannot make assumptions about
/// the receiver's filesystem view.
pub(crate) fn prepare_remote_spawn_args(
    params: &Value,
    from_agent: &str,
    default_cwd: &Path,
) -> Result<RemoteSpawnArgs, String> {
    loopal_ipc::cross_hub::validate_spawn_payload(params)?;

    let name = params["name"]
        .as_str()
        .ok_or("missing 'name' field")?
        .to_string();
    let model = params["model"].as_str().map(String::from);
    let prompt = params["prompt"].as_str().map(String::from);
    let permission_mode = params["permission_mode"].as_str().map(String::from);
    if let Some(ref pm) = permission_mode {
        // P3: receiver currently applies caller's mode without arbitration.
        tracing::warn!(
            permission_mode = %pm,
            agent = %name,
            "cross-hub spawn: applying caller's permission_mode hint without local policy arbitration (P3)"
        );
    }
    let agent_type = params["agent_type"].as_str().map(String::from);
    // Clamp depth >= 1: a malicious cross-hub caller could send `depth: 0`
    // to make the child appear root-equivalent and bypass the receiver's
    // depth-based spawn-tool filter (`build_depth_tool_filter`). Cross-hub
    // is at least one hop, so depth must be > 0.
    let depth = params["depth"].as_u64().map(|v| v as u32).map(|d| d.max(1));
    let parent = match params["parent"].as_str() {
        Some(s) => {
            // Cross-hub parent MUST be a well-formed remote QualifiedAddress
            // (hub/agent). Empty segments make `parse` silently fall back to
            // a local address (preserving the input verbatim as the agent
            // name) — that would let a malicious caller route completions to
            // an unintended local agent. Reject anything that doesn't survive
            // the round-trip as a remote address.
            let parsed = loopal_protocol::QualifiedAddress::parse(s);
            if !parsed.is_remote() {
                return Err(format!(
                    "cross-hub spawn 'parent' must be a remote QualifiedAddress (hub/agent), got '{s}'"
                ));
            }
            Some(s.to_string())
        }
        None => Some(from_agent.to_string()),
    };

    Ok(RemoteSpawnArgs {
        name,
        cwd: default_cwd.to_string_lossy().into_owned(),
        model,
        prompt,
        permission_mode,
        agent_type,
        depth,
        parent,
    })
}
