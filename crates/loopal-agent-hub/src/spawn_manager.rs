//! Spawn manager — Hub spawns agent processes and registers their stdio.

use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{info, warn};

use loopal_ipc::connection::{Connection, Incoming};
use loopal_protocol::{AgentEvent, AgentEventPayload};

use crate::hub::Hub;

/// Spawn a real agent process, initialize, start, and register in Hub.
pub async fn spawn_and_register(
    hub: Arc<Mutex<Hub>>,
    name: String,
    cwd: String,
    model: Option<String>,
    prompt: Option<String>,
    parent: Option<String>,
) -> Result<String, String> {
    info!(agent = %name, parent = ?parent, "spawn: forking process");
    let agent_proc = loopal_agent_client::AgentProcess::spawn(None)
        .await
        .map_err(|e| format!("failed to spawn agent process: {e}"))?;

    // If init or start fails, kill the orphaned process before returning error.
    let client = loopal_agent_client::AgentClient::new(agent_proc.transport());
    info!(agent = %name, "spawn: initializing IPC");
    if let Err(e) = client.initialize().await {
        warn!(agent = %name, error = %e, "spawn: init failed, killing orphan");
        let _ = agent_proc.shutdown().await;
        return Err(format!("agent initialize failed: {e}"));
    }
    info!(agent = %name, "spawn: starting agent");
    if let Err(e) = client
        .start_agent(
            std::path::Path::new(&cwd),
            model.as_deref(),
            Some("act"),
            prompt.as_deref(),
            None,
            false,
            None,
        )
        .await
    {
        warn!(agent = %name, error = %e, "spawn: start failed, killing orphan");
        let _ = agent_proc.shutdown().await;
        return Err(format!("agent/start failed: {e}"));
    }

    let (conn, incoming_rx) = client.into_parts();
    let agent_id = register_agent_connection(
        hub,
        &name,
        conn,
        incoming_rx,
        parent.as_deref(),
        model.as_deref(),
    )
    .await;

    // Supervised process cleanup: when agent exits, task completes.
    // AgentProcess::drop will kill the child (kill_on_drop) if not exited.
    tokio::spawn(async move {
        let _ = agent_proc.wait().await;
    });

    info!(agent = %name, "agent spawned and registered via Hub");
    Ok(agent_id)
}

/// Register a pre-built Connection as a named agent in Hub.
/// Registration completes synchronously; IO loop runs in background.
pub async fn register_agent_connection(
    hub: Arc<Mutex<Hub>>,
    name: &str,
    conn: Arc<Connection>,
    incoming_rx: tokio::sync::mpsc::Receiver<Incoming>,
    parent: Option<&str>,
    model: Option<&str>,
) -> String {
    let agent_id = uuid::Uuid::new_v4().to_string();

    {
        let mut h = hub.lock().await;
        // Validate parent exists (if specified)
        if let Some(p) = parent {
            if !h.registry.agents.contains_key(p) {
                warn!(agent = %name, parent = %p, "parent not found, registering as orphan");
            }
        }
        if let Err(e) =
            h.registry
                .register_connection_with_parent(name, conn.clone(), parent, model)
        {
            warn!(agent = %name, error = %e, "registration failed");
            return agent_id;
        }
        h.registry
            .set_lifecycle(name, crate::AgentLifecycle::Running);
    }
    info!(agent = %name, "agent registered in Hub");

    crate::agent_io::spawn_io_loop(hub.clone(), name, conn, incoming_rx, false);

    {
        let h = hub.lock().await;
        let event = AgentEvent::root(AgentEventPayload::SubAgentSpawned {
            name: name.to_string(),
            agent_id: agent_id.clone(),
            parent: parent.map(String::from),
            model: model.map(String::from),
        });
        if h.registry.event_sender().try_send(event).is_err() {
            tracing::debug!(agent = %name, "SubAgentSpawned event dropped");
        }
    }

    agent_id
}
