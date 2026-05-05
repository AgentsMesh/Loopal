use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::hub::Hub;

use super::register::register_agent_connection;

/// Spawn a real agent process, initialize, start, and register in Hub.
#[allow(clippy::too_many_arguments)]
pub async fn spawn_and_register(
    hub: Arc<Mutex<Hub>>,
    name: String,
    cwd: String,
    model: Option<String>,
    prompt: Option<String>,
    parent: Option<String>,
    permission_mode: Option<String>,
    agent_type: Option<String>,
    depth: Option<u32>,
    fork_context: Option<serde_json::Value>,
    no_sandbox: bool,
) -> Result<String, String> {
    if parent.is_some() {
        let h = hub.lock().await;
        let sub_count = h.registry.sub_agent_count();
        if sub_count >= h.max_total_agents as usize {
            return Err(format!(
                "Spawn budget exhausted ({sub_count}/{} sub-agents). \
                 Complete the task with your own tools.",
                h.max_total_agents
            ));
        }
    }

    info!(agent = %name, parent = ?parent, "spawn: forking process");
    let agent_proc = loopal_agent_client::AgentProcess::spawn(None)
        .await
        .map_err(|e| format!("failed to spawn agent process: {e}"))?;

    let client = loopal_agent_client::AgentClient::new(agent_proc.transport());
    info!(agent = %name, "spawn: initializing IPC");
    if let Err(e) = client.initialize().await {
        warn!(agent = %name, error = %e, "spawn: init failed, killing orphan");
        let _ = agent_proc.shutdown().await;
        return Err(format!("agent initialize failed: {e}"));
    }
    info!(agent = %name, "spawn: starting agent");
    let model_for_registry = model.clone();
    let session_id = match client
        .start_agent(&loopal_agent_client::StartAgentParams {
            cwd: std::path::PathBuf::from(&cwd),
            model,
            mode: Some("act".to_string()),
            prompt,
            permission_mode,
            no_sandbox,
            resume: None,
            lifecycle: Some("ephemeral".to_string()),
            agent_type,
            depth,
            fork_context,
        })
        .await
    {
        Ok(sid) => Some(sid),
        Err(e) => {
            warn!(agent = %name, error = %e, "spawn: start failed, killing orphan");
            let _ = agent_proc.shutdown().await;
            return Err(format!("agent/start failed: {e}"));
        }
    };

    let (conn, incoming_rx) = client.into_parts();
    match register_agent_connection(
        hub,
        &name,
        conn,
        incoming_rx,
        parent.as_deref(),
        model_for_registry.as_deref(),
        session_id.as_deref(),
    )
    .await
    {
        Ok(agent_id) => {
            tokio::spawn(async move {
                let _ = agent_proc.wait().await;
            });
            info!(agent = %name, "agent spawned and registered via Hub");
            Ok(agent_id)
        }
        Err(e) => {
            warn!(agent = %name, error = %e, "registration failed, killing orphan");
            let _ = agent_proc.shutdown().await;
            Err(e)
        }
    }
}
