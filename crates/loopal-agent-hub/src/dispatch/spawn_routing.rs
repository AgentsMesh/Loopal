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

fn normalize_target_hub_value(value: Option<&Value>) -> Result<Option<String>, String> {
    let Some(v) = value else {
        return Ok(None);
    };
    let target = v
        .as_str()
        .ok_or_else(|| format!("'target_hub' must be a string, got: {v}"))?
        .trim();
    if target.is_empty() {
        Ok(None)
    } else {
        Ok(Some(target.to_string()))
    }
}

/// In-hub spawn entry point. If `target_hub` is set, forward to MetaHub
/// after rejecting any filesystem-coupled fields (cwd / fork_context /
/// resume). Otherwise spawn locally.
pub async fn handle_spawn_agent(
    hub: &Arc<Mutex<Hub>>,
    params: Value,
    from_agent: &str,
) -> Result<Value, String> {
    if let Some(target) = normalize_target_hub_value(params.get("target_hub"))? {
        // Mirror the agent-name check in cross_hub_forward::preflight: a
        // hub identifier with '/' would be ambiguous with QualifiedAddress
        // multi-hop encoding (`hub-c/hub-d/agent`) — reject up front.
        if target.contains('/') {
            return Err(format!(
                "'target_hub' cannot contain '/' (cross-hub address encoding), got: {target}"
            ));
        }
        let own_hub = hub
            .lock()
            .await
            .uplink
            .as_ref()
            .map(|u| u.hub_name().to_string());
        if !is_self_target(own_hub.as_deref(), &target) {
            let mut cross_params = params;
            if let Some(obj) = cross_params.as_object_mut() {
                obj.insert("target_hub".into(), Value::String(target));
            }
            return super::cross_hub_forward::forward_cross_hub_spawn(
                hub,
                cross_params,
                from_agent,
            )
            .await;
        }
        let mut local_params = params;
        if let Some(obj) = local_params.as_object_mut() {
            obj.remove("target_hub");
        }
        return spawn_local(hub, local_params, from_agent).await;
    }
    let mut local_params = params;
    if let Some(obj) = local_params.as_object_mut() {
        obj.remove("target_hub");
    }
    spawn_local(hub, local_params, from_agent).await
}

// reason: a hub targeting itself would pre-register a shadow then route back
// through MetaHub into its own registry, colliding as "already registered" and
// orphaning a forked process. Self-target must spawn locally — same registry,
// no MetaHub round-trip.
fn is_self_target(own_hub: Option<&str>, target: &str) -> bool {
    own_hub == Some(target)
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
    let decision_mode = params["decision_mode"].as_str().map(String::from);
    let agent_type = params["agent_type"].as_str().map(String::from);
    let depth = params["depth"].as_u64().map(|v| v as u32);
    let fork_context = params.get("fork_context").cloned();
    let no_sandbox = params["no_sandbox"].as_bool().unwrap_or(false);
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
        decision_mode,
        agent_type,
        depth,
        fork_context,
        no_sandbox,
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
    let args = super::spawn_prepare::prepare_remote_spawn_args(&params, from_agent, &default_cwd)?;
    info!(agent = %args.name, parent = ?args.parent, "handle_spawn_remote_agent start");
    spawn_via_manager(
        hub.clone(),
        args.name,
        args.cwd,
        args.model,
        args.prompt,
        args.parent,
        args.permission_mode,
        args.decision_mode,
        args.agent_type,
        args.depth,
        None,
        args.no_sandbox,
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
    decision_mode: Option<String>,
    agent_type: Option<String>,
    depth: Option<u32>,
    fork_context: Option<Value>,
    no_sandbox: bool,
) -> Result<Value, String> {
    let name_clone = name.clone();
    let handle = tokio::spawn(async move {
        crate::spawn_manager::spawn_and_register(
            hub,
            name_clone,
            cwd,
            model,
            prompt,
            parent,
            permission_mode,
            decision_mode,
            agent_type,
            depth,
            fork_context,
            no_sandbox,
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{is_self_target, normalize_target_hub_value};

    #[test]
    fn own_hub_equals_target_is_self() {
        assert!(is_self_target(Some("hub-a"), "hub-a"));
    }

    #[test]
    fn different_hub_is_not_self() {
        assert!(!is_self_target(Some("hub-a"), "hub-b"));
    }

    #[test]
    fn no_uplink_is_never_self() {
        assert!(!is_self_target(None, "hub-a"));
    }

    #[test]
    fn normalize_target_hub_value_treats_empty_as_absent() {
        assert_eq!(normalize_target_hub_value(None).unwrap(), None);
        assert_eq!(normalize_target_hub_value(Some(&json!(""))).unwrap(), None);
        assert_eq!(
            normalize_target_hub_value(Some(&json!("   "))).unwrap(),
            None
        );
        assert_eq!(
            normalize_target_hub_value(Some(&json!(" hub-b "))).unwrap(),
            Some("hub-b".into())
        );
    }

    #[test]
    fn normalize_target_hub_value_rejects_non_string() {
        let err = normalize_target_hub_value(Some(&json!(42))).expect_err("must reject");
        assert!(err.contains("target_hub") && err.contains("string"));
    }
}
