use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::info;

use crate::hub::Hub;
use crate::types::RegisteredAgent;

use super::PreparedSpawn;
#[cfg(test)]
pub(super) use super::process::ProcessFuture;
pub(super) use super::process::{
    PreparedAgentProcess, PreparedControl, SpawnProcess, WorkflowProcessOwner,
};
use super::register::register_prepared_spawn;

pub(crate) async fn spawn_and_register(
    hub: Arc<Mutex<Hub>>,
    prepared: PreparedSpawn,
) -> Result<String, String> {
    spawn_and_register_with_execution(hub, prepared)
        .await
        .map(|value| value.agent_id)
}

pub(crate) async fn spawn_and_register_with_execution(
    hub: Arc<Mutex<Hub>>,
    prepared: PreparedSpawn,
) -> Result<RegisteredAgent, String> {
    let process = fork_process(&hub, &prepared).await?;
    initialize_and_register(hub, prepared, process).await
}

pub(super) async fn fork_process(
    hub: &Arc<Mutex<Hub>>,
    prepared: &PreparedSpawn,
) -> Result<loopal_agent_client::AgentProcess, String> {
    fork_process_if(hub, prepared, || Ok(())).await
}

pub(super) async fn fork_process_if(
    hub: &Arc<Mutex<Hub>>,
    prepared: &PreparedSpawn,
    admit: impl FnOnce() -> Result<(), String>,
) -> Result<loopal_agent_client::AgentProcess, String> {
    info!(agent = %prepared.name, parent = ?prepared.parent, "spawn: forking process");
    super::fork::authorized(hub, prepared, || {
        admit()?;
        loopal_agent_client::AgentProcess::spawn_now(None)
            .map_err(|error| format!("failed to spawn agent process: {error}"))
    })
    .await
}

pub(super) async fn prepare_and_register_process<P: SpawnProcess>(
    hub: Arc<Mutex<Hub>>,
    prepared: PreparedSpawn,
    process: P,
) -> Result<PreparedAgentProcess<P>, String> {
    let client = loopal_agent_client::AgentClient::new(process.transport());
    if let Err(error) = client.initialize().await {
        let _ = process.shutdown().await;
        return Err(format!("agent initialize failed: {error}"));
    }
    let session_id = uuid::Uuid::new_v4().to_string();
    let start_params = prepared.start_params(session_id.clone());
    let (connection, incoming) = client.into_parts();
    let registered = match register_prepared_spawn(
        hub,
        &prepared,
        connection.clone(),
        incoming,
        Some(&session_id),
    )
    .await
    {
        Ok(value) => value,
        Err(error) => {
            let _ = process.shutdown().await;
            return Err(error);
        }
    };
    Ok(PreparedAgentProcess::new(
        process,
        connection,
        registered,
        session_id,
        start_params,
        prepared.name,
    ))
}

async fn initialize_and_register<P: SpawnProcess>(
    hub: Arc<Mutex<Hub>>,
    prepared: PreparedSpawn,
    process: P,
) -> Result<RegisteredAgent, String> {
    let prepared = prepare_and_register_process(hub.clone(), prepared, process).await?;
    let registered = prepared.registered.clone();
    if let Err(error) = prepared.activate().await {
        crate::finish::finish_and_deliver_exact(
            &hub,
            &prepared.name,
            loopal_protocol::AgentCompletion::new("start_failed", Some(error.clone())),
            &prepared.connection,
            &registered.execution,
        )
        .await;
        prepared.shutdown().await;
        return Err(error);
    }
    hub.lock().await.registry.set_lifecycle(
        &registered.execution.address.agent,
        crate::AgentLifecycle::Running,
    );
    prepared.spawn_wait();
    Ok(registered)
}

#[cfg(test)]
#[path = "spawn_tests.rs"]
mod tests;
