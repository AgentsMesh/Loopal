//! `SharedSession` + `ClientHandle` — multi-client observers attached
//! to a single agent loop.
//!
//! Split out of [`crate::session_hub`] so that file stays focused on
//! the server-wide registry and storage singletons. The two types are
//! tightly coupled (every `SharedSession` holds a `Vec<ClientHandle>`)
//! but logically separate from the registry that owns sessions.

use std::sync::{Arc, Weak};

use tokio::sync::Mutex;

use loopal_agent::AgentShared;
use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::InterruptSignal;
use loopal_runtime::agent_input::AgentInput;

/// A connected client handle within a shared session.
pub struct ClientHandle {
    pub id: String,
    pub connection: Arc<Connection<Listening>>,
    /// True if this is the primary client (handles permissions/questions).
    pub is_primary: bool,
}

#[derive(Clone)]
pub(crate) struct ClientConnectionLease {
    pub id: String,
    pub connection: Arc<Connection<Listening>>,
    pub is_primary: bool,
}

/// A shared session that multiple clients can observe.
pub struct SharedSession {
    pub session_id: String,
    pub clients: Mutex<Vec<ClientHandle>>,
    /// Channel to send input into the agent loop.
    pub input_tx: tokio::sync::mpsc::Sender<AgentInput>,
    /// Interrupt signal shared with the agent loop.
    pub interrupt: InterruptSignal,
    pub interrupt_tx: Arc<tokio::sync::watch::Sender<u64>>,
    /// Weak reference to the agent's runtime context, set after
    /// `agent_setup` completes. `agent/state_snapshot` reads this to
    /// produce a snapshot for Hub-side ViewState rebuild. Held as `Weak`
    /// so this reference doesn't extend `AgentShared`'s lifetime —
    /// otherwise the embedded `hub_connection` would prevent stdio EOF
    /// and block server shutdown.
    pub agent_shared: Mutex<Option<Weak<AgentShared>>>,
    pub(crate) pending_workflow_terminals:
        crate::workflow_terminal_pending::WorkflowTerminalPending,
}

impl SharedSession {
    /// Create a placeholder session (for bootstrapping before session_id is known).
    pub fn placeholder(
        input_tx: tokio::sync::mpsc::Sender<AgentInput>,
        interrupt: InterruptSignal,
        interrupt_tx: Arc<tokio::sync::watch::Sender<u64>>,
    ) -> Self {
        Self::new(String::new(), input_tx, interrupt, interrupt_tx)
    }

    pub fn new(
        session_id: String,
        input_tx: tokio::sync::mpsc::Sender<AgentInput>,
        interrupt: InterruptSignal,
        interrupt_tx: Arc<tokio::sync::watch::Sender<u64>>,
    ) -> Self {
        Self {
            session_id,
            clients: Mutex::new(Vec::new()),
            input_tx,
            interrupt,
            interrupt_tx,
            agent_shared: Mutex::new(None),
            pending_workflow_terminals:
                crate::workflow_terminal_pending::WorkflowTerminalPending::new(),
        }
    }

    /// Inject the typed `AgentShared` after `agent_setup` finishes.
    /// Stored as `Weak` so this reference doesn't extend `AgentShared`'s
    /// lifetime (see field doc).
    pub async fn set_agent_shared(&self, shared: &Arc<AgentShared>) {
        *self.agent_shared.lock().await = Some(Arc::downgrade(shared));
    }

    /// Snapshot per-agent state for `agent/state_snapshot` IPC.
    /// Returns `None` if no agent has been bound yet, or if the agent
    /// has already exited and its `AgentShared` was dropped.
    pub async fn snapshot_agent_state(&self) -> Option<loopal_protocol::AgentStateSnapshot> {
        let weak = self.agent_shared.lock().await.clone()?;
        let shared = weak.upgrade()?;
        Some(shared.snapshot_state().await)
    }

    /// Add a client to this session. First client becomes primary.
    pub async fn add_client(&self, id: String, connection: Arc<Connection<Listening>>) {
        let mut clients = self.clients.lock().await;
        let is_primary = clients.is_empty();
        clients.push(ClientHandle {
            id,
            connection,
            is_primary,
        });
    }

    /// Remove a client. If the removed client was primary, promote the next.
    pub async fn remove_client(&self, client_id: &str) {
        let mut clients = self.clients.lock().await;
        let was_primary = clients
            .iter()
            .find(|c| c.id == client_id)
            .is_some_and(|c| c.is_primary);
        clients.retain(|c| c.id != client_id);
        if was_primary && let Some(first) = clients.first_mut() {
            first.is_primary = true;
            tracing::info!(client = %first.id, "promoted to primary");
        }
    }

    /// Get the primary client's connection (for permission/question routing).
    pub async fn primary_connection(&self) -> Option<Arc<Connection<Listening>>> {
        self.clients
            .lock()
            .await
            .iter()
            .find(|c| c.is_primary)
            .map(|c| c.connection.clone())
    }

    /// Get all client connections (for event broadcast).
    pub async fn all_connections(&self) -> Vec<Arc<Connection<Listening>>> {
        self.clients
            .lock()
            .await
            .iter()
            .map(|c| c.connection.clone())
            .collect()
    }

    pub(crate) async fn connection_leases(&self) -> Vec<ClientConnectionLease> {
        self.clients
            .lock()
            .await
            .iter()
            .map(|client| ClientConnectionLease {
                id: client.id.clone(),
                connection: client.connection.clone(),
                is_primary: client.is_primary,
            })
            .collect()
    }

    /// Remove only the exact failed connection generations captured by a send.
    pub(crate) async fn remove_failed_connections(&self, failed: &[ClientConnectionLease]) {
        let mut clients = self.clients.lock().await;
        clients.retain(|client| {
            let remove = failed.iter().any(|lease| {
                lease.id == client.id && Arc::ptr_eq(&lease.connection, &client.connection)
            });
            if remove {
                tracing::info!(client = %client.id, "removed failed event connection");
            }
            !remove
        });
        let has_primary = clients.iter().any(|c| c.is_primary);
        if !has_primary && let Some(first) = clients.first_mut() {
            first.is_primary = true;
            tracing::info!(client = %first.id, "promoted to primary (event delivery cleanup)");
        }
    }
}
