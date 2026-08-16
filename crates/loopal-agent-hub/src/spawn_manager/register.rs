use std::sync::Arc;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_protocol::QualifiedAddress;
use tokio::sync::{Mutex, mpsc};

use crate::hub::Hub;
use crate::types::{AgentOrigin, AgentRuntimeFacts, RegisteredAgent};

use super::prepared::{PreparedSpawn, SpawnRequestLease};
use super::register_exact::{Registration, register};

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
    register_agent_connection_with_execution(
        hub,
        name,
        conn,
        incoming_rx,
        parent,
        model,
        session_id,
        notify_parent_on_completion,
    )
    .await
    .map(|registered| registered.agent_id)
}

pub(crate) async fn register_prepared_spawn(
    hub: Arc<Mutex<Hub>>,
    prepared: &PreparedSpawn,
    conn: Arc<Connection<Listening>>,
    incoming_rx: mpsc::Receiver<Incoming>,
    session_id: Option<&str>,
) -> Result<RegisteredAgent, String> {
    register(
        hub,
        conn,
        incoming_rx,
        Registration {
            name: prepared.name.clone(),
            request_lease: prepared.request_lease.clone(),
            parent: prepared.parent.clone(),
            expected_parent: prepared.parent_execution.clone(),
            model: Some(prepared.authority.model.clone()),
            session_id: session_id.map(String::from),
            notify_parent_on_completion: prepared.notify_parent_on_completion,
            mark_running: false,
            facts: prepared.runtime_facts(session_id),
        },
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn register_agent_connection_with_execution(
    hub: Arc<Mutex<Hub>>,
    name: &str,
    conn: Arc<Connection<Listening>>,
    incoming_rx: mpsc::Receiver<Incoming>,
    parent: Option<&str>,
    model: Option<&str>,
    session_id: Option<&str>,
    notify_parent_on_completion: bool,
) -> Result<RegisteredAgent, String> {
    let parent = parent.map(QualifiedAddress::parse);
    let facts = {
        let locked = hub.lock().await;
        derive_runtime_facts(&locked, parent.as_ref(), session_id)?
    };
    let expected_parent = facts.parent.clone();
    register(
        hub,
        conn,
        incoming_rx,
        Registration {
            name: name.to_string(),
            request_lease: SpawnRequestLease::Internal,
            parent,
            expected_parent,
            model: model.map(String::from),
            session_id: session_id.map(String::from),
            notify_parent_on_completion,
            mark_running: true,
            facts,
        },
    )
    .await
}

fn derive_runtime_facts(
    hub: &Hub,
    parent: Option<&QualifiedAddress>,
    session_id: Option<&str>,
) -> Result<AgentRuntimeFacts, String> {
    let Some(parent) = parent else {
        return Ok(AgentRuntimeFacts {
            origin: AgentOrigin::ManagedChild,
            cwd: hub.default_cwd.clone(),
            root_cwd: hub.default_cwd.clone(),
            root: loopal_protocol::ROOT_AGENT_NAME.to_string(),
            parent: None,
            depth: 1,
            session_id: session_id.map(String::from),
            workflow_permission_causation: None,
            workflow_attempt_capability_digest: None,
            workflow_completion_result_limit: None,
            spawn: hub.root_spawn_authority(),
        });
    };
    if !parent.is_local() {
        return Err("local child registration requires a local parent".into());
    }
    let parent_execution = hub
        .registry
        .current_execution(&parent.agent)
        .ok_or_else(|| format!("parent agent '{}' is not active", parent.agent))?;
    let parent_facts = hub
        .registry
        .runtime_facts(&parent_execution)
        .ok_or_else(|| format!("parent agent '{}' has no runtime authority", parent.agent))?;
    Ok(AgentRuntimeFacts {
        origin: AgentOrigin::ManagedChild,
        cwd: parent_facts.cwd.clone(),
        root_cwd: parent_facts.root_cwd.clone(),
        root: parent_facts.root.clone(),
        parent: Some(parent_execution),
        depth: parent_facts.depth.saturating_add(1),
        session_id: session_id
            .map(String::from)
            .or_else(|| parent_facts.session_id.clone()),
        workflow_permission_causation: parent_facts.workflow_permission_causation.clone(),
        workflow_attempt_capability_digest: parent_facts.workflow_attempt_capability_digest,
        workflow_completion_result_limit: parent_facts.workflow_completion_result_limit,
        spawn: parent_facts.spawn.clone(),
    })
}
