use std::sync::Arc;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use tokio::sync::{Mutex, mpsc};
use tracing::info;

use crate::authoritative_events::PreparedAuthoritativeEvent;
use crate::hub::Hub;
use crate::types::{AgentExecutionRef, RegisteredAgent};

use super::completion_bridge::spawn_completion_bridge;

pub(super) struct SpawnAdmission {
    pub(super) hub: Arc<Mutex<Hub>>,
    pub(super) name: String,
    pub(super) connection: Arc<Connection<Listening>>,
    pub(super) incoming: mpsc::Receiver<Incoming>,
    pub(super) completion_rx: mpsc::Receiver<loopal_protocol::Envelope>,
    pub(super) delivery: PreparedAuthoritativeEvent,
    pub(super) parent_name: String,
    pub(super) parent_generation: Option<u64>,
    pub(super) registered: RegisteredAgent,
    pub(super) cleanup: AdmissionCleanup,
}

pub(super) struct AdmissionCleanup {
    hub: Arc<Mutex<Hub>>,
    connection: Arc<Connection<Listening>>,
    execution: AgentExecutionRef,
    armed: bool,
}

impl AdmissionCleanup {
    pub(super) fn new(
        hub: Arc<Mutex<Hub>>,
        connection: Arc<Connection<Listening>>,
        execution: AgentExecutionRef,
    ) -> Self {
        Self {
            hub,
            connection,
            execution,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for AdmissionCleanup {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let hub = self.hub.clone();
        let connection = self.connection.clone();
        let execution = self.execution.clone();
        let panicking = std::thread::panicking();
        // Admission may be dropped while a workflow preparation is cancelled.
        // Keep exact cleanup alive independently of the cancelled future.
        runtime.spawn(async move {
            let mcp = {
                let mut locked = hub.lock().await;
                if panicking {
                    locked.shutdown_signal.notify_one();
                }
                locked.clear_permission_grants(&execution);
                locked.spawn_registry.unregister_exact(&execution);
                locked.registry.unregister_exact(&execution);
                locked.mcp_service.clone()
            };
            mcp.on_agent_detach(&execution).await;
            close_bounded(&connection).await;
        });
    }
}

impl SpawnAdmission {
    pub(super) async fn complete(mut self) -> Result<RegisteredAgent, String> {
        if let Err(error) = self.delivery.deliver().await {
            tracing::error!(agent = %self.name, %error,
                "SubAgentSpawned admission failed; unregistering agent");
            self.cleanup_exact(&self.registered.execution).await;
            return Err(error.to_string());
        }
        let dispatcher = Arc::new(crate::dispatch::build_hub_dispatcher(self.hub.clone()));
        let mut locked = self.hub.lock().await;
        if !locked.registry.owns_lease(&self.registered.execution) {
            drop(locked);
            close_bounded(&self.connection).await;
            return Err(format!(
                "agent '{}' reconnected before spawn admission completed",
                self.name
            ));
        }
        if self.parent_generation.is_some_and(|generation| {
            !locked
                .registry
                .owns_active_generation(&self.parent_name, generation)
        }) {
            locked.registry.unregister_exact(&self.registered.execution);
            drop(locked);
            close_bounded(&self.connection).await;
            return Err(format!(
                "parent agent '{}' reconnected before spawn admission completed",
                self.parent_name
            ));
        }
        let facts = locked
            .registry
            .runtime_facts(&self.registered.execution)
            .cloned()
            .ok_or_else(|| "child runtime authority was lost".to_string())?;
        let topology = locked.spawn_registry.clone();
        let mcp = locked.mcp_service.clone();
        if !topology.register_exact(
            self.registered.execution.clone(),
            facts.cwd.clone(),
            facts.parent,
        ) {
            locked.registry.unregister_exact(&self.registered.execution);
            drop(locked);
            close_bounded(&self.connection).await;
            return Err("stale child topology registration".into());
        }
        drop(locked);
        mcp.on_agent_attach(self.registered.execution.clone(), facts.cwd)
            .await;
        self.ensure_active_after_mcp(&topology, &mcp).await?;
        spawn_completion_bridge(&self.name, self.connection.clone(), self.completion_rx);
        crate::agent_io::spawn_io_loop_exact(
            self.hub.clone(),
            dispatcher,
            &self.name,
            self.connection.clone(),
            self.incoming,
            self.registered.execution.clone(),
        );
        info!(agent = %self.name, "agent registered in Hub");
        self.cleanup.disarm();
        Ok(self.registered)
    }

    async fn ensure_active_after_mcp(
        &self,
        topology: &crate::spawn_registry::SpawnRegistry,
        mcp: &crate::mcp_service::HubMcpService,
    ) -> Result<(), String> {
        if !self
            .hub
            .lock()
            .await
            .registry
            .owns_active_lease(&self.registered.execution)
        {
            topology.unregister_exact(&self.registered.execution);
            mcp.on_agent_detach(&self.registered.execution).await;
            close_bounded(&self.connection).await;
            return Err("child reconnected during MCP admission".into());
        }
        Ok(())
    }

    async fn cleanup_exact(&self, execution: &AgentExecutionRef) {
        let removed = self.hub.lock().await.registry.unregister_exact(execution);
        if removed {
            close_bounded(&self.connection).await;
        }
    }
}

pub(super) async fn close_bounded(connection: &Connection<Listening>) {
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), connection.close()).await;
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;
