use std::path::{Path, PathBuf};
use std::sync::Arc;

use loopal_config::SandboxPolicy;
use loopal_decision_api::DecisionMode;
use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::QualifiedAddress;
use loopal_tool_api::PermissionMode;
use serde_json::Value;

use crate::hub::Hub;
use crate::spawn_manager::{PreparedSpawn, SpawnRequestLease};
use crate::types::SpawnAuthority;

#[derive(Debug)]
pub struct RemoteSpawnArgs {
    pub name: String,
    pub cwd: PathBuf,
    pub model: String,
    pub prompt: Option<String>,
    pub permission_mode: String,
    pub decision_mode: String,
    pub sandbox_policy: String,
    pub agent_type: Option<String>,
    pub depth: u32,
    pub parent: String,
    pub no_sandbox: bool,
}

pub fn prepare_remote_spawn_args(
    params: &Value,
    default_cwd: &Path,
) -> Result<RemoteSpawnArgs, String> {
    loopal_ipc::cross_hub::validate_forwarded_spawn_payload(params)?;
    let parent = required_string(params, "parent")?;
    let parsed_parent = QualifiedAddress::parse(&parent);
    if !parsed_parent.is_remote() || parsed_parent.agent.is_empty() {
        return Err("cross-hub spawn parent must be a remote QualifiedAddress".into());
    }
    let cwd = default_cwd
        .canonicalize()
        .map_err(|_| "destination Hub cwd must exist".to_string())?;
    if !cwd.is_dir() {
        return Err("destination Hub cwd must be a directory".into());
    }
    Ok(RemoteSpawnArgs {
        name: required_string(params, "name")?,
        cwd,
        model: required_string(params, "model")?,
        prompt: optional_string(params, "prompt")?,
        permission_mode: required_string(params, "permission_mode")?,
        decision_mode: required_string(params, "decision_mode")?,
        sandbox_policy: required_string(params, "sandbox_policy")?,
        agent_type: optional_string(params, "agent_type")?,
        depth: params["depth"]
            .as_u64()
            .expect("shared validation checked depth") as u32,
        parent,
        no_sandbox: params["no_sandbox"]
            .as_bool()
            .expect("shared validation checked no_sandbox"),
    })
}

pub(crate) fn prepare_remote_spawn(
    params: &Value,
    hub: &Hub,
    connection: Arc<Connection<Listening>>,
) -> Result<PreparedSpawn, String> {
    let args = prepare_remote_spawn_args(params, &hub.default_cwd)?;
    if args.depth > hub.max_agent_depth {
        return Err(format!(
            "agent depth limit exceeded ({}/{})",
            args.depth, hub.max_agent_depth
        ));
    }
    let incoming_permission = args
        .permission_mode
        .parse::<PermissionMode>()
        .map_err(|error| error.to_string())?;
    let incoming_decision = args
        .decision_mode
        .parse::<DecisionMode>()
        .map_err(|error| error.to_string())?;
    let incoming_sandbox = args.sandbox_policy.parse::<SandboxPolicy>()?;
    let ceiling = hub.root_spawn_authority();
    let authority = SpawnAuthority {
        model: args.model,
        permission_mode: strictest_permission(incoming_permission, ceiling.permission_mode),
        decision_mode: strictest_decision(incoming_decision, ceiling.decision_mode),
        sandbox_policy: strictest_sandbox(incoming_sandbox, ceiling.sandbox_policy),
    };
    let root_cwd = loopal_git::repo_root(&args.cwd)
        .and_then(|root| root.canonicalize().ok())
        .unwrap_or_else(|| args.cwd.clone());
    Ok(PreparedSpawn {
        name: args.name.clone(),
        request_lease: SpawnRequestLease::TrustedMetaHub(connection),
        cwd: args.cwd,
        prompt: args.prompt,
        parent: Some(QualifiedAddress::parse(&args.parent)),
        parent_execution: None,
        authority,
        agent_type: args.agent_type,
        depth: args.depth,
        fork_context: None,
        workflow_permission_causation: None,
        workflow_attempt_capability: None,
        workflow_completion_result_limit: None,
        notify_parent_on_completion: true,
        root_cwd,
        root: args.name,
    })
}

fn required_string(params: &Value, field: &str) -> Result<String, String> {
    params[field]
        .as_str()
        .map(String::from)
        .ok_or_else(|| format!("cross-hub spawn '{field}' must be a string"))
}

fn optional_string(params: &Value, field: &str) -> Result<Option<String>, String> {
    match params.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(format!("cross-hub spawn '{field}' must be a string")),
    }
}

fn strictest_permission(left: PermissionMode, right: PermissionMode) -> PermissionMode {
    use PermissionMode::{AskAnyWrite, AskDangerous, Bypass};
    match (left, right) {
        (AskAnyWrite, _) | (_, AskAnyWrite) => AskAnyWrite,
        (AskDangerous, _) | (_, AskDangerous) => AskDangerous,
        (Bypass, Bypass) => Bypass,
    }
}

fn strictest_decision(left: DecisionMode, right: DecisionMode) -> DecisionMode {
    if left == DecisionMode::Manual || right == DecisionMode::Manual {
        DecisionMode::Manual
    } else {
        right
    }
}

fn strictest_sandbox(left: SandboxPolicy, right: SandboxPolicy) -> SandboxPolicy {
    use SandboxPolicy::{DefaultWrite, Disabled, ReadOnly};
    match (left, right) {
        (ReadOnly, _) | (_, ReadOnly) => ReadOnly,
        (DefaultWrite, _) | (_, DefaultWrite) => DefaultWrite,
        (Disabled, Disabled) => Disabled,
    }
}
