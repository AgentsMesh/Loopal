use std::path::Path;

use serde_json::Value;

#[derive(Debug)]
pub struct RemoteSpawnArgs {
    pub name: String,
    pub cwd: String,
    pub model: Option<String>,
    pub prompt: Option<String>,
    pub permission_mode: Option<String>,
    pub decision_mode: Option<String>,
    pub agent_type: Option<String>,
    pub depth: Option<u32>,
    pub parent: Option<String>,
    pub no_sandbox: bool,
}

pub fn prepare_remote_spawn_args(
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
    // reason: cross-hub child has no path to a UI; force Bypass + Manual so misconfigured
    // callers get a working—if permissive—agent rather than 30s timeout denials.
    let requested_mode = params["permission_mode"].as_str();
    let requested_decision = params["decision_mode"].as_str();
    if requested_mode.is_some_and(|m| m != "bypass")
        || requested_decision.is_some_and(|d| d != "manual")
    {
        tracing::warn!(
            agent = %params["name"].as_str().unwrap_or(""),
            requested_mode = ?requested_mode,
            requested_decision = ?requested_decision,
            "cross-hub spawn: clamping to Bypass + Manual (no UI on remote hub)"
        );
    }
    let permission_mode = Some("bypass".to_string());
    let decision_mode = Some("manual".to_string());
    let agent_type = params["agent_type"].as_str().map(String::from);
    let depth = params["depth"].as_u64().map(|v| v as u32).map(|d| d.max(1));
    let parent = match params["parent"].as_str() {
        Some(s) => {
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
        decision_mode,
        agent_type,
        depth,
        parent,
        no_sandbox: params["no_sandbox"].as_bool().unwrap_or(false),
    })
}
