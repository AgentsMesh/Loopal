use std::path::{Path, PathBuf};

use serde_json::Value;

use super::spawn_authority_fields::{
    optional_nonempty_string, optional_string, reject_fields, require_optional_bool,
    require_optional_string, require_optional_u32, required_nonempty_string,
};
use crate::request_principal::AgentPrincipal;
use crate::spawn_manager::{PreparedSpawn, SpawnRequestLease};

pub(super) fn prepare_cross_hub_payload(
    params: &Value,
    principal: &AgentPrincipal,
    max_depth: u32,
) -> Result<Value, String> {
    loopal_ipc::cross_hub::validate_spawn_payload(params)?;
    reject_fields(params, &["parent", "session_id", "lifecycle"])?;
    let depth = derived_depth(principal, max_depth)?;
    require_optional_u32(params, "depth", depth)?;
    require_optional_string(
        params,
        "permission_mode",
        &principal.spawn.permission_mode.to_string(),
    )?;
    require_optional_string(
        params,
        "decision_mode",
        &principal.spawn.decision_mode.to_string(),
    )?;
    require_optional_bool(
        params,
        "no_sandbox",
        principal.spawn.sandbox_policy == loopal_config::SandboxPolicy::Disabled,
    )?;
    require_optional_string(
        params,
        "sandbox_policy",
        &principal.spawn.sandbox_policy.to_string(),
    )?;
    let name = required_nonempty_string(params, "name")?;
    if name.contains('/') {
        return Err("agent name cannot contain '/'".into());
    }
    let model =
        optional_nonempty_string(params, "model")?.unwrap_or_else(|| principal.spawn.model.clone());
    let mut prepared = params.clone();
    let object = prepared
        .as_object_mut()
        .ok_or_else(|| "spawn params must be an object".to_string())?;
    object.insert("name".into(), Value::String(name));
    object.insert("model".into(), Value::String(model));
    object.insert("depth".into(), Value::from(depth));
    object.insert(
        "permission_mode".into(),
        Value::String(principal.spawn.permission_mode.to_string()),
    );
    object.insert(
        "decision_mode".into(),
        Value::String(principal.spawn.decision_mode.to_string()),
    );
    object.insert(
        "sandbox_policy".into(),
        Value::String(principal.spawn.sandbox_policy.to_string()),
    );
    object.insert(
        "no_sandbox".into(),
        Value::Bool(principal.spawn.sandbox_policy == loopal_config::SandboxPolicy::Disabled),
    );
    Ok(prepared)
}

pub(super) fn prepare_local(
    params: &Value,
    principal: &AgentPrincipal,
    max_depth: u32,
) -> Result<PreparedSpawn, String> {
    let object = params
        .as_object()
        .ok_or_else(|| "spawn params must be an object".to_string())?;
    reject_fields(
        params,
        &["resume", "session_id", "lifecycle", "sandbox_policy"],
    )?;
    let name = required_nonempty_string(params, "name")?;
    if name.contains('/') {
        return Err("agent name cannot contain '/'".into());
    }
    let depth = derived_depth(principal, max_depth)?;
    match object.get("parent") {
        None | Some(Value::Null) => {}
        Some(Value::String(parent)) if parent == &principal.execution.address.to_string() => {}
        Some(_) => return Err("spawn parent must match the authenticated caller".into()),
    }
    require_optional_u32(params, "depth", depth)?;
    require_optional_string(
        params,
        "permission_mode",
        &principal.spawn.permission_mode.to_string(),
    )?;
    require_optional_string(
        params,
        "decision_mode",
        &principal.spawn.decision_mode.to_string(),
    )?;
    require_optional_bool(
        params,
        "no_sandbox",
        principal.spawn.sandbox_policy == loopal_config::SandboxPolicy::Disabled,
    )?;
    let cwd = canonical_child_cwd(params.get("cwd"), &principal.cwd, &principal.root_cwd)?;
    let model =
        optional_nonempty_string(params, "model")?.unwrap_or_else(|| principal.spawn.model.clone());
    let notify_parent_on_completion = match object.get("notify_parent_on_completion") {
        None => true,
        Some(value) => value
            .as_bool()
            .ok_or_else(|| "notify_parent_on_completion must be a boolean".to_string())?,
    };
    let mut authority = principal.spawn.clone();
    authority.model = model;
    Ok(PreparedSpawn {
        name,
        request_lease: SpawnRequestLease::Agent(principal.execution.clone()),
        cwd,
        prompt: optional_string(params, "prompt")?,
        parent: Some(principal.execution.address.clone()),
        parent_execution: Some(principal.execution.clone()),
        authority,
        agent_type: optional_string(params, "agent_type")?,
        depth,
        fork_context: params.get("fork_context").cloned(),
        workflow_permission_causation: principal.workflow_permission_causation.clone(),
        workflow_attempt_capability: None,
        workflow_completion_result_limit: None,
        notify_parent_on_completion,
        root_cwd: principal.root_cwd.clone(),
        root: principal.root.clone(),
    })
}

fn derived_depth(principal: &AgentPrincipal, max_depth: u32) -> Result<u32, String> {
    let depth = principal
        .depth
        .checked_add(1)
        .ok_or_else(|| "agent depth overflow".to_string())?;
    if depth > max_depth {
        return Err(format!("agent depth limit exceeded ({depth}/{max_depth})"));
    }
    Ok(depth)
}

fn canonical_child_cwd(
    requested: Option<&Value>,
    parent_cwd: &Path,
    root_cwd: &Path,
) -> Result<PathBuf, String> {
    let path = match requested {
        None | Some(Value::Null) => parent_cwd.to_path_buf(),
        Some(Value::String(path)) if !path.is_empty() => {
            let requested = PathBuf::from(path);
            if requested.is_absolute() {
                requested
            } else {
                parent_cwd.join(requested)
            }
        }
        Some(_) => return Err("cwd must be a non-empty string".into()),
    };
    let canonical = path
        .canonicalize()
        .map_err(|_| "spawn cwd must be an existing directory".to_string())?;
    if !canonical.is_dir() || !canonical.starts_with(root_cwd) {
        return Err("spawn cwd is outside the authenticated project root".into());
    }
    Ok(canonical)
}
