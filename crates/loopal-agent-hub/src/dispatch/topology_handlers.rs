//! Hub topology query handlers.

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::hub::AgentHub;

pub async fn handle_agent_info(
    hub: &Arc<Mutex<AgentHub>>,
    params: Value,
) -> Result<Value, String> {
    let name = params["name"].as_str().ok_or("missing 'name'")?;
    let h = hub.lock().await;

    // Check finished_outputs first (agent may already be unregistered)
    let output = h.finished_outputs.get(name).cloned();

    if let Some(info) = h.agent_info(name) {
        Ok(json!({
            "name": info.name,
            "parent": info.parent,
            "children": info.children,
            "lifecycle": format!("{:?}", info.lifecycle),
            "model": info.model,
            "output": output,
        }))
    } else if let Some(ref out) = output {
        // Agent unregistered but output cached
        Ok(json!({
            "name": name,
            "lifecycle": "Finished",
            "output": out,
        }))
    } else {
        Err(format!("agent '{name}' not found"))
    }
}

pub async fn handle_topology(hub: &Arc<Mutex<AgentHub>>) -> Result<Value, String> {
    Ok(hub.lock().await.topology_snapshot())
}
