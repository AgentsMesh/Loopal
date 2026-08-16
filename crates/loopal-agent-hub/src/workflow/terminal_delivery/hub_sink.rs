use std::sync::Arc;

use loopal_ipc::protocol::methods;
use loopal_protocol::{
    DEFAULT_WORKFLOW_TERMINAL_RPC_TIMEOUT, WorkflowTerminalDisposition,
    WorkflowTerminalNotification,
};
use tokio::sync::Mutex;

use super::WorkflowTerminalSink;
use crate::Hub;
use crate::workflow::{WorkflowOwner, owner_for_managed_root};

pub(crate) struct HubWorkflowTerminalSink {
    hub: Arc<Mutex<Hub>>,
}

impl HubWorkflowTerminalSink {
    pub(crate) fn new(hub: Arc<Mutex<Hub>>) -> Self {
        Self { hub }
    }

    async fn exact_connection(
        &self,
        owner: &WorkflowOwner,
    ) -> Result<
        (
            crate::types::AgentExecutionRef,
            Arc<loopal_ipc::connection::Connection<loopal_ipc::connection::Listening>>,
        ),
        String,
    > {
        let hub = self.hub.lock().await;
        let execution = hub
            .registry
            .current_execution(&owner.root_agent.agent)
            .ok_or_else(|| "workflow root Agent is not connected".to_string())?;
        let facts = hub
            .registry
            .runtime_facts(&execution)
            .ok_or_else(|| "workflow root Agent has no authenticated runtime facts".to_string())?;
        if owner_for_managed_root(&execution, facts).as_ref() != Ok(owner) {
            return Err("workflow root Agent authority changed".into());
        }
        let connection = hub
            .registry
            .exact_connection(&execution)
            .ok_or_else(|| "workflow root Agent connection changed".to_string())?;
        Ok((execution, connection))
    }

    async fn still_exact(
        &self,
        owner: &WorkflowOwner,
        execution: &crate::types::AgentExecutionRef,
    ) -> bool {
        let hub = self.hub.lock().await;
        hub.registry
            .runtime_facts(execution)
            .and_then(|facts| owner_for_managed_root(execution, facts).ok())
            .as_ref()
            == Some(owner)
            && hub.registry.exact_connection(execution).is_some()
    }
}

#[async_trait::async_trait]
impl WorkflowTerminalSink for HubWorkflowTerminalSink {
    async fn deliver(
        &self,
        owner: &WorkflowOwner,
        notification: WorkflowTerminalNotification,
    ) -> Result<WorkflowTerminalDisposition, String> {
        notification
            .validate()
            .map_err(|error| format!("invalid workflow terminal notification: {error:?}"))?;
        let (execution, connection) = self.exact_connection(owner).await?;
        let params = serde_json::to_value(notification)
            .map_err(|error| format!("encode workflow terminal notification: {error}"))?;
        let value = tokio::time::timeout(
            DEFAULT_WORKFLOW_TERMINAL_RPC_TIMEOUT,
            connection.send_request(methods::AGENT_WORKFLOW_TERMINAL.name, params),
        )
        .await
        .map_err(|_| "workflow terminal RPC timed out".to_string())?
        .map_err(|error| format!("workflow terminal RPC failed: {error}"))?;
        if !self.still_exact(owner, &execution).await {
            return Err("workflow root Agent changed before terminal acknowledgement".into());
        }
        serde_json::from_value(value)
            .map_err(|error| format!("decode workflow terminal disposition: {error}"))
    }
}

#[cfg(test)]
#[path = "hub_sink_tests.rs"]
mod tests;
