//! Pure-function preparation step for `hub/spawn_remote_agent`. Extracted
//! from the IPC handler so unit tests can verify field handling — including
//! that the receiver's `default_cwd` is used, not any value the caller sent —
//! without depending on `spawn_manager` (which spawns real subprocesses).
//!
//! `permission_mode` is always clamped to `bypass` for cross-hub agents
//! because the receiver hub is assumed headless (no UI clients) — any
//! non-Bypass mode would only manifest as 30s timeout denials. `depth`
//! is clamped `>= 1`. Cross-network deployment must add receiver-side
//! policy arbitration (per-hub allow-list, depth-floor from receiver
//! config) before becoming safe.
//!
//! TODO(cross-hub-trust): `no_sandbox` is currently passed through verbatim
//! from the caller hub. Unlike `permission_mode` this has no UI dependency,
//! so no clamp is needed on functional grounds — but it grants caller-side
//! ability to disable the receiver's OS sandbox. Same arbitration story
//! as `permission_mode`: receiver-side allow-list / per-hub policy must
//! gate this before cross-hub is production-safe.

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
    pub no_sandbox: bool,
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
    // Cross-hub agents have no path to reach UI clients for permission.
    // The Hub-side `pending_relay::handle_agent_permission` fast-denies
    // when no local UI is registered (worker hubs are headless), so any
    // non-Bypass mode would manifest as 30s timeout denials. Clamp here
    // so misconfigured callers get a working — if permissive — agent.
    let requested = params["permission_mode"].as_str();
    if let Some(pm) = requested
        && pm != "bypass"
    {
        tracing::warn!(
            agent = %params["name"].as_str().unwrap_or(""),
            requested = %pm,
            "cross-hub spawn: clamping permission_mode to bypass (no UI on remote hub)"
        );
    }
    let permission_mode = Some("bypass".to_string());
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
        no_sandbox: params["no_sandbox"].as_bool().unwrap_or(false),
    })
}
