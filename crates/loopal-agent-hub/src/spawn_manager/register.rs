use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tracing::{info, warn};

use loopal_ipc::connection::{Connection, Incoming};
use loopal_protocol::{AgentEvent, AgentEventPayload, Envelope, QualifiedAddress};

use crate::hub::Hub;

use super::completion_bridge::spawn_completion_bridge;

/// Register a pre-built Connection as a named agent in Hub.
/// Performs spawn budget check atomically with registration.
pub async fn register_agent_connection(
    hub: Arc<Mutex<Hub>>,
    name: &str,
    conn: Arc<Connection>,
    incoming_rx: mpsc::Receiver<Incoming>,
    parent: Option<&str>,
    model: Option<&str>,
    session_id: Option<&str>,
) -> Result<String, String> {
    let agent_id = uuid::Uuid::new_v4().to_string();

    let (completion_tx, completion_rx) = mpsc::channel::<Envelope>(32);

    // String parent comes in qualified-or-bare form depending on caller
    // (cross-hub spawn provides "hub/agent", local spawn provides "agent").
    let parent_addr = parent.map(loopal_protocol::QualifiedAddress::parse);

    {
        let mut h = hub.lock().await;

        // Atomic with registration — no TOCTOU.
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
