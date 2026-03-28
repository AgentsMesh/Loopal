//! Hub wait_agent handler — waits for a spawned agent to finish.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::Mutex;
use tracing::info;

use crate::hub::AgentHub;

/// Wait for an agent to finish. Timeout: 10 minutes.
pub async fn handle_wait_agent(hub: &Arc<Mutex<AgentHub>>, params: Value) -> Result<Value, String> {
    let name = params["name"]
        .as_str()
        .ok_or("missing 'name' field")?
        .to_string();
    info!(agent = %name, "handle_wait_agent start");

    let mut rx = {
        let mut h = hub.lock().await;

        // Check cached output first (agent already finished before we got here)
        if let Some(output) = h.finished_outputs.get(&name) {
            info!(agent = %name, "handle_wait_agent: found cached output");
            return Ok(json!({"output": output}));
        }

        // Agent still running — create watcher
        if h.get_agent_connection(&name).is_none() {
            info!(agent = %name, "handle_wait_agent: not found");
            return Ok(json!({"output": "agent not found or already finished"}));
        }
        let rx = h.watch_completion(&name);
        if rx.borrow().is_some() {
            let output = rx.borrow().as_ref().cloned().unwrap_or_default();
            info!(agent = %name, "handle_wait_agent: already completed");
            return Ok(json!({"output": output}));
        }
        rx
    }; // Lock released.

    info!(agent = %name, "handle_wait_agent: waiting");
    let wait = async {
        while rx.changed().await.is_ok() {
            if let Some(output) = rx.borrow().as_ref() {
                return Some(output.clone());
            }
        }
        None
    };
    match tokio::time::timeout(Duration::from_secs(600), wait).await {
        Ok(Some(output)) => {
            info!(agent = %name, "handle_wait_agent: completed");
            Ok(json!({"output": output}))
        }
        Ok(None) => {
            info!(agent = %name, "handle_wait_agent: terminated");
            Ok(json!({"output": "agent terminated"}))
        }
        Err(_) => {
            info!(agent = %name, "handle_wait_agent: timed out (10min)");
            Ok(json!({"output": "(agent timed out)", "timed_out": true}))
        }
    }
}
