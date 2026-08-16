use std::sync::Arc;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_protocol::{AgentEvent, AgentEventPayload, Envelope, QualifiedAddress};
use tokio::sync::{Mutex, mpsc};

use crate::authoritative_events::PreparedAuthoritativeEvent;
use crate::hub::Hub;
use crate::types::{AgentExecutionRef, AgentRuntimeFacts, RegisteredAgent};

use super::SpawnRequestLease;
use super::admission::SpawnAdmission;
#[cfg(test)]
use super::admission::close_bounded;

pub(super) struct Registration {
    pub(super) name: String,
    pub(super) request_lease: SpawnRequestLease,
    pub(super) parent: Option<QualifiedAddress>,
    pub(super) expected_parent: Option<AgentExecutionRef>,
    pub(super) model: Option<String>,
    pub(super) session_id: Option<String>,
    pub(super) notify_parent_on_completion: bool,
    pub(super) mark_running: bool,
    pub(super) facts: AgentRuntimeFacts,
}

pub(super) async fn register(
    hub: Arc<Mutex<Hub>>,
    connection: Arc<Connection<Listening>>,
    incoming: mpsc::Receiver<Incoming>,
    registration: Registration,
) -> Result<RegisteredAgent, String> {
    let agent_id = uuid::Uuid::new_v4().to_string();
    let (completion_tx, completion_rx) = mpsc::channel::<Envelope>(32);
    let (registered, delivery, parent_name, parent_generation) = {
        let mut locked = hub.lock().await;
        if !registration.request_lease.is_current(&locked) {
            return Err("spawn requester connection lease is stale".into());
        }
        check_budget(&locked, &registration)?;
        let execution = locked
            .registry
            .register_connection_with_exact_parent_execution(
                &registration.name,
                connection.clone(),
                registration.parent.clone(),
                registration.expected_parent.as_ref(),
                registration.model.as_deref(),
                Some(completion_tx),
                registration.notify_parent_on_completion,
            )
            .map_err(|error| format!("agent registration failed: {error}"))?;
        install_runtime_authority(&mut locked, &execution, registration.facts.clone())?;
        if registration.mark_running {
            locked
                .registry
                .set_lifecycle(&registration.name, crate::AgentLifecycle::Running);
        }
        let (delivery, parent_name, parent_generation) =
            prepare_event(&locked, &registration, agent_id.clone());
        (
            RegisteredAgent {
                agent_id: agent_id.clone(),
                execution,
            },
            delivery,
            parent_name,
            parent_generation,
        )
    };
    let admission = SpawnAdmission {
        hub: hub.clone(),
        name: registration.name,
        connection: connection.clone(),
        incoming,
        completion_rx,
        delivery,
        parent_name,
        parent_generation,
        cleanup: super::admission::AdmissionCleanup::new(
            hub.clone(),
            connection.clone(),
            registered.execution.clone(),
        ),
        registered,
    };
    // Keep admission owned by the caller.  A detached coordinator can outlive
    // a cancelled workflow preparation and register a worker after its lease
    // has already been terminalized.
    admission.complete().await
}

fn install_runtime_authority(
    hub: &mut Hub,
    execution: &AgentExecutionRef,
    facts: AgentRuntimeFacts,
) -> Result<(), String> {
    if !hub.registry.set_runtime_facts(execution, facts) {
        hub.registry.unregister_exact(execution);
        return Err("agent runtime authority registration failed".into());
    }
    Ok(())
}

#[cfg(test)]
async fn await_admission(
    hub: Arc<Mutex<Hub>>,
    connection: Arc<Connection<Listening>>,
    cleanup_execution: AgentExecutionRef,
    coordinator: tokio::task::JoinHandle<Result<RegisteredAgent, String>>,
) -> Result<RegisteredAgent, String> {
    match coordinator.await {
        Ok(result) => result,
        Err(error) => {
            let mut locked = hub.lock().await;
            locked.shutdown_signal.notify_one();
            let removed = locked.registry.unregister_exact(&cleanup_execution);
            drop(locked);
            if removed {
                close_bounded(&connection).await;
            }
            Err(format!(
                "SubAgentSpawned admission coordinator failed: {error}"
            ))
        }
    }
}

fn check_budget(hub: &Hub, registration: &Registration) -> Result<(), String> {
    if registration.parent.is_none() {
        return Ok(());
    }
    let count = hub.registry.sub_agent_count();
    if count >= hub.max_total_agents as usize {
        tracing::warn!(agent = %registration.name, count, "spawn budget exhausted");
        return Err(format!(
            "Spawn budget exhausted ({count}/{} sub-agents). Complete the task with your own tools.",
            hub.max_total_agents
        ));
    }
    Ok(())
}

fn prepare_event(
    hub: &Hub,
    registration: &Registration,
    agent_id: String,
) -> (PreparedAuthoritativeEvent, String, Option<u64>) {
    let target = registration
        .parent
        .clone()
        .unwrap_or_else(|| QualifiedAddress::local(loopal_protocol::ROOT_AGENT_NAME));
    let parent_name = target.agent.clone();
    let parent_generation = if target.is_local() {
        registration
            .expected_parent
            .as_ref()
            .map(|execution| execution.connection_generation)
            .or_else(|| hub.registry.generation(&parent_name))
    } else {
        None
    };
    let mut event = AgentEvent::named(
        target,
        AgentEventPayload::SubAgentSpawned(loopal_protocol::SubAgentSpawn {
            name: registration.name.clone(),
            agent_id,
            parent: registration.parent.clone(),
            model: registration.model.clone(),
            session_id: registration.session_id.clone(),
        }),
    );
    event.routing_generation = parent_generation;
    (
        PreparedAuthoritativeEvent::from_hub(hub, event),
        parent_name,
        parent_generation,
    )
}

#[cfg(test)]
#[path = "register_cancellation_tests.rs"]
mod cancellation_tests;
#[cfg(test)]
#[path = "register_exact_tests.rs"]
mod tests;
