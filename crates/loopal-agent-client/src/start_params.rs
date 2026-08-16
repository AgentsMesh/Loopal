use std::path::PathBuf;

use serde_json::Value;

#[derive(Debug, Default)]
pub struct StartAgentParams {
    pub cwd: PathBuf,
    pub model: Option<String>,
    pub mode: Option<String>,
    pub prompt: Option<String>,
    pub permission_mode: Option<String>,
    pub decision_mode: Option<String>,
    pub no_sandbox: bool,
    pub sandbox_policy: Option<String>,
    pub session_id: Option<String>,
    pub workflow_permission_causation: Option<loopal_protocol::WorkflowPermissionCausation>,
    pub workflow_attempt_capability: Option<loopal_protocol::WorkflowAttemptCapability>,
    pub workflow_completion_result_limit: Option<u32>,
    pub resume: Option<String>,
    pub lifecycle: Option<String>,
    pub agent_type: Option<String>,
    pub depth: Option<u32>,
    pub fork_context: Option<Value>,
}

pub fn encode(p: &StartAgentParams) -> Value {
    let mut params = serde_json::json!({
        "cwd": p.cwd.to_string_lossy(),
        "model": p.model,
        "mode": p.mode,
        "prompt": p.prompt,
        "permission_mode": p.permission_mode,
        "decision_mode": p.decision_mode,
        "no_sandbox": p.no_sandbox,
        "sandbox_policy": p.sandbox_policy,
        "session_id": p.session_id,
        "workflow_permission_causation": p.workflow_permission_causation,
        "workflow_attempt_capability": p.workflow_attempt_capability,
        "workflow_completion_result_limit": p.workflow_completion_result_limit,
        "resume": p.resume,
        "lifecycle": p.lifecycle,
        "agent_type": p.agent_type,
        "depth": p.depth,
    });
    if let Some(ref fc) = p.fork_context {
        params["fork_context"] = fc.clone();
    }
    params
}
