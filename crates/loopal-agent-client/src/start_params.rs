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
    pub resume: Option<String>,
    pub lifecycle: Option<String>,
    pub agent_type: Option<String>,
    pub depth: Option<u32>,
    pub fork_context: Option<Value>,
}

pub(crate) fn encode(p: &StartAgentParams) -> Value {
    let mut params = serde_json::json!({
        "cwd": p.cwd.to_string_lossy(),
        "model": p.model,
        "mode": p.mode,
        "prompt": p.prompt,
        "permission_mode": p.permission_mode,
        "decision_mode": p.decision_mode,
        "no_sandbox": p.no_sandbox,
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
