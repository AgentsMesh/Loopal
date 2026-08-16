use std::path::PathBuf;
use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{QualifiedAddress, UiCapabilities};

use crate::types::{AgentExecutionRef, AgentOrigin, AgentRuntimeFacts, SpawnAuthority};

#[derive(Clone)]
pub(crate) enum HubRequestPrincipal {
    Ui(UiPrincipal),
    Agent(AgentPrincipal),
    TrustedMetaHub(TrustedMetaHubPrincipal),
    Internal,
}

#[derive(Clone)]
pub(crate) struct UiPrincipal {
    pub(crate) lease_id: String,
    pub(crate) name: String,
    pub(crate) capabilities: UiCapabilities,
    connection: Arc<Connection<Listening>>,
}

impl UiPrincipal {
    pub(crate) fn new(
        lease_id: String,
        name: String,
        capabilities: UiCapabilities,
        connection: Arc<Connection<Listening>>,
    ) -> Self {
        Self {
            lease_id,
            name,
            capabilities,
            connection,
        }
    }

    pub(crate) fn matches_connection(&self, connection: &Arc<Connection<Listening>>) -> bool {
        Arc::ptr_eq(&self.connection, connection)
    }

    pub(crate) fn is_current_permission_ui(&self, hub: &crate::Hub) -> bool {
        hub.ui
            .client_lease(&self.lease_id)
            .is_some_and(|(name, capabilities, connection)| {
                name == self.name
                    && capabilities == self.capabilities
                    && capabilities.supports(loopal_protocol::UiCapability::Permission)
                    && self.matches_connection(&connection)
            })
    }
}

#[derive(Clone)]
pub(crate) struct AgentPrincipal {
    pub(crate) execution: AgentExecutionRef,
    pub(crate) origin: AgentOrigin,
    pub(crate) cwd: PathBuf,
    pub(crate) root_cwd: PathBuf,
    pub(crate) root: String,
    pub(crate) depth: u32,
    pub(crate) session_id: Option<String>,
    pub(crate) workflow_permission_causation: Option<loopal_protocol::WorkflowPermissionCausation>,
    pub(crate) spawn: SpawnAuthority,
}

impl AgentPrincipal {
    pub(crate) fn new(execution: AgentExecutionRef, facts: AgentRuntimeFacts) -> Self {
        Self {
            execution,
            origin: facts.origin,
            cwd: facts.cwd,
            root_cwd: facts.root_cwd,
            root: facts.root,
            depth: facts.depth,
            session_id: facts.session_id,
            workflow_permission_causation: facts.workflow_permission_causation,
            spawn: facts.spawn,
        }
    }

    pub(crate) fn address(&self) -> &QualifiedAddress {
        &self.execution.address
    }

    pub(crate) fn is_managed(&self) -> bool {
        matches!(
            self.origin,
            AgentOrigin::ManagedRoot | AgentOrigin::ManagedChild
        )
    }
}

#[derive(Clone)]
pub(crate) struct TrustedMetaHubPrincipal {
    connection: Arc<Connection<Listening>>,
}

impl TrustedMetaHubPrincipal {
    pub(crate) fn new(connection: Arc<Connection<Listening>>) -> Self {
        Self { connection }
    }

    pub(crate) fn matches_connection(&self, connection: &Arc<Connection<Listening>>) -> bool {
        Arc::ptr_eq(&self.connection, connection)
    }

    pub(crate) fn connection(&self) -> Arc<Connection<Listening>> {
        self.connection.clone()
    }
}
