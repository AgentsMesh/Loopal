use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tracing::{info, warn};

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_protocol::{AgentEvent, AgentEventPayload, Envelope, QualifiedAddress};

use crate::authoritative_events::PreparedAuthoritativeEvent;
use crate::hub::Hub;

use super::completion_bridge::spawn_completion_bridge;

/// Register a pre-built Connection as a named agent in Hub.
/// Performs spawn budget check atomically with registration.
pub async fn register_agent_connection(
    hub: Arc<Mutex<Hub>>,
    name: &str,
    conn: Arc<Connection<Listening>>,
    incoming_rx: mpsc::Receiver<Incoming>,
    parent: Option<&str>,
    model: Option<&str>,
    session_id: Option<&str>,
) -> Result<String, String> {
    register_agent_connection_with_policy(
        hub,
        name,
        conn,
        incoming_rx,
        parent,
        model,
        session_id,
        true,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn register_agent_connection_with_policy(
    hub: Arc<Mutex<Hub>>,
    name: &str,
    conn: Arc<Connection<Listening>>,
    incoming_rx: mpsc::Receiver<Incoming>,
    parent: Option<&str>,
    model: Option<&str>,
    session_id: Option<&str>,
    notify_parent_on_completion: bool,
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
        if let Err(e) = h.registry.register_connection_with_parent_policy(
            name,
            conn.clone(),
            parent_addr.clone(),
            model,
            Some(completion_tx),
            notify_parent_on_completion,
        ) {
            warn!(agent = %name, error = %e, "registration failed");
            return Err(format!("agent registration failed: {e}"));
        }
        h.registry
            .set_lifecycle(name, crate::AgentLifecycle::Running);
    }
    let registered_conn = conn.clone();
    let (mut delivery, parent_agent, parent_generation) = {
        let h = hub.lock().await;
        // Routed to the parent agent so the parent's ViewStateReducer
        // appends `name` to its `children` field. Parent defaults to
        // root agent when unspecified (top-level spawn).
        let parent_agent = parent_addr
            .as_ref()
            .map(|p| p.agent.clone())
            .unwrap_or_else(|| loopal_protocol::ROOT_AGENT_NAME.to_string());
        let event = AgentEvent::named(
            QualifiedAddress::local(&parent_agent),
            AgentEventPayload::SubAgentSpawned(loopal_protocol::SubAgentSpawn {
                name: name.to_string(),
                agent_id: agent_id.clone(),
                parent: parent_addr.clone(),
                model: model.map(String::from),
                session_id: session_id.map(String::from),
            }),
        );
        let event = h.registry.prepare_generation_event(&parent_agent, event);
        (
            PreparedAuthoritativeEvent::from_hub(&h, event),
            parent_agent.clone(),
            h.registry.generation(&parent_agent),
        )
    };
    let delivery_hub = hub.clone();
    let delivery_name = name.to_string();
    let cleanup_conn = registered_conn.clone();
    let coordinator = tokio::spawn(async move {
        if let Err(error) = delivery.deliver().await {
            tracing::error!(
                agent = %delivery_name,
                %error,
                "SubAgentSpawned admission failed; unregistering agent"
            );
            let removed = delivery_hub
                .lock()
                .await
                .registry
                .unregister_connection_if_current(&delivery_name, &cleanup_conn);
            if removed {
                let _ =
                    tokio::time::timeout(std::time::Duration::from_secs(2), cleanup_conn.close())
                        .await;
            }
            return Err(error.to_string());
        }
        // Do not let an already-buffered agent/completed overtake the spawn
        // observation. The completion bridge and IO owner become runnable only
        // after SubAgentSpawned is durably admitted.
        let dispatcher =
            std::sync::Arc::new(crate::dispatch::build_hub_dispatcher(delivery_hub.clone()));
        let mut locked = delivery_hub.lock().await;
        if parent_generation.is_some_and(|generation| {
            !locked
                .registry
                .owns_active_generation(&parent_agent, generation)
        }) {
            locked
                .registry
                .unregister_connection_if_current(&delivery_name, &cleanup_conn);
            drop(locked);
            let _ =
                tokio::time::timeout(std::time::Duration::from_secs(2), cleanup_conn.close()).await;
            return Err(format!(
                "parent agent '{parent_agent}' reconnected before spawn admission completed"
            ));
        }
        spawn_completion_bridge(&delivery_name, cleanup_conn.clone(), completion_rx);
        crate::agent_io::spawn_io_loop(
            delivery_hub.clone(),
            dispatcher,
            &delivery_name,
            cleanup_conn,
            incoming_rx,
        );
        drop(locked);
        info!(agent = %delivery_name, "agent registered in Hub");
        Ok(())
    });
    match coordinator.await {
        Ok(result) => result?,
        Err(error) => {
            hub.lock().await.shutdown_signal.notify_one();
            let removed = hub
                .lock()
                .await
                .registry
                .unregister_connection_if_current(name, &registered_conn);
            if removed {
                let _ = tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    registered_conn.close(),
                )
                .await;
            }
            return Err(format!(
                "SubAgentSpawned admission coordinator failed: {error}"
            ));
        }
    }
    Ok(agent_id)
}
