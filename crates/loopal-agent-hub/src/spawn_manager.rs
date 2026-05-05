//! Spawn manager — Hub spawns agent processes and registers their stdio.
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tracing::{info, warn};

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload, Envelope, QualifiedAddress};

use crate::hub::Hub;

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
    // Pre-check budget BEFORE spawning process to avoid orphans.
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
            lifecycle: Some("ephemeral".to_string()), // sub-agents always exit on idle
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

/// Register a pre-built Connection as a named agent in Hub.
/// Performs spawn budget check atomically with registration.
pub async fn register_agent_connection(
    hub: Arc<Mutex<Hub>>,
    name: &str,
    conn: Arc<Connection>,
    incoming_rx: tokio::sync::mpsc::Receiver<Incoming>,
    parent: Option<&str>,
    model: Option<&str>,
    session_id: Option<&str>,
) -> Result<String, String> {
    let agent_id = uuid::Uuid::new_v4().to_string();

    // Completion channel: Hub writes here when this agent's children finish.
    // Bridge task forwards to the agent process via IPC.
    let (completion_tx, completion_rx) = mpsc::channel::<Envelope>(32);

    // String parent comes in qualified-or-bare form depending on caller
    // (cross-hub spawn provides "hub/agent", local spawn provides "agent").
    let parent_addr = parent.map(loopal_protocol::QualifiedAddress::parse);

    {
        let mut h = hub.lock().await;

        // Budget check (atomic with registration — no TOCTOU).
        // Count only sub-agents (those with a parent), excluding root "main".
        if parent.is_some() {
            let sub_count = h.registry.sub_agent_count();
            if sub_count >= h.max_total_agents as usize {
                warn!(agent = %name, count = sub_count, "spawn budget exhausted");
                return Err(format!(
                    "Spawn budget exhausted ({sub_count}/{} sub-agents). \
                     Complete the task with your own tools.",
                    h.max_total_agents
                ));
            }
        }

        if let Some(p) = &parent_addr
            && p.is_local()
            && !h.registry.agents.contains_key(&p.agent)
        {
            warn!(agent = %name, parent = %p, "parent not found");
        }
        if let Err(e) = h.registry.register_connection_with_parent(
            name,
            conn.clone(),
            parent_addr.clone(),
            model,
            Some(completion_tx),
        ) {
            warn!(agent = %name, error = %e, "registration failed");
            return Err(format!("agent registration failed: {e}"));
        }
        h.registry
            .set_lifecycle(name, crate::AgentLifecycle::Running);
    }
    info!(agent = %name, "agent registered in Hub");

    spawn_completion_bridge(name, conn.clone(), completion_rx);
    crate::agent_io::spawn_io_loop(hub.clone(), name, conn, incoming_rx);

    {
        let h = hub.lock().await;
        // Routed to the parent agent so the parent's ViewStateReducer
        // appends `name` to its `children` field. Parent defaults to
        // root "main" when unspecified (top-level spawn).
        let parent_agent = parent_addr
            .as_ref()
            .map(|p| p.agent.clone())
            .unwrap_or_else(|| "main".to_string());
        let event = AgentEvent::named(
            QualifiedAddress::local(parent_agent),
            AgentEventPayload::SubAgentSpawned {
                name: name.to_string(),
                agent_id: agent_id.clone(),
                parent: parent_addr.clone(),
                model: model.map(String::from),
                session_id: session_id.map(String::from),
            },
        );
        if h.registry.event_sender().try_send(event).is_err() {
            tracing::warn!(agent = %name, "SubAgentSpawned event dropped (channel full)");
        }
    }
    Ok(agent_id)
}

/// Bridge: reads from Hub-internal channel, forwards to agent via IPC notification.
pub fn spawn_completion_bridge(
    name: &str,
    conn: Arc<Connection>,
    mut rx: mpsc::Receiver<Envelope>,
) {
    let n = name.to_string();
    tokio::spawn(async move {
        while let Some(envelope) = rx.recv().await {
            let params = match serde_json::to_value(&envelope) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(agent = %n, error = %e, "completion envelope serialization failed");
                    continue;
                }
            };
            if let Err(e) = conn
                .send_notification(methods::AGENT_MESSAGE.name, params)
                .await
            {
                tracing::warn!(agent = %n, error = %e, "completion notification IPC send failed");
            }
        }
    });
}
