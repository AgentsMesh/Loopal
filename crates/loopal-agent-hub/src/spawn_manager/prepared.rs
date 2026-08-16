use std::path::PathBuf;
use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::QualifiedAddress;

use crate::hub::Hub;
use crate::types::{AgentExecutionRef, AgentOrigin, AgentRuntimeFacts, SpawnAuthority};

#[derive(Clone)]
pub(crate) enum SpawnRequestLease {
    Agent(AgentExecutionRef),
    TrustedMetaHub(Arc<Connection<Listening>>),
    Internal,
}

impl SpawnRequestLease {
    pub(crate) fn is_current(&self, hub: &Hub) -> bool {
        match self {
            Self::Agent(execution) => hub.registry.owns_active_lease(execution),
            Self::TrustedMetaHub(connection) => hub.is_active_uplink_connection(connection),
            Self::Internal => true,
        }
    }
}

pub(crate) struct PreparedSpawn {
    pub(crate) name: String,
    pub(crate) request_lease: SpawnRequestLease,
    pub(crate) cwd: PathBuf,
    pub(crate) prompt: Option<String>,
    pub(crate) parent: Option<QualifiedAddress>,
    pub(crate) parent_execution: Option<AgentExecutionRef>,
    pub(crate) authority: SpawnAuthority,
    pub(crate) agent_type: Option<String>,
    pub(crate) depth: u32,
    pub(crate) fork_context: Option<serde_json::Value>,
    pub(crate) workflow_permission_causation: Option<loopal_protocol::WorkflowPermissionCausation>,
    pub(crate) workflow_attempt_capability: Option<loopal_protocol::WorkflowAttemptCapability>,
    pub(crate) workflow_completion_result_limit: Option<u32>,
    pub(crate) notify_parent_on_completion: bool,
    pub(crate) root_cwd: PathBuf,
    pub(crate) root: String,
}

impl PreparedSpawn {
    pub(crate) fn start_params(&self, session_id: String) -> loopal_agent_client::StartAgentParams {
        loopal_agent_client::StartAgentParams {
            cwd: self.cwd.clone(),
            model: Some(self.authority.model.clone()),
            mode: Some("act".into()),
            prompt: self.prompt.clone(),
            permission_mode: Some(self.authority.permission_mode.to_string()),
            decision_mode: Some(self.authority.decision_mode.to_string()),
            no_sandbox: self.authority.sandbox_policy == loopal_config::SandboxPolicy::Disabled,
            sandbox_policy: Some(self.authority.sandbox_policy.to_string()),
            session_id: Some(session_id),
            workflow_permission_causation: self.workflow_permission_causation.clone(),
            workflow_attempt_capability: self.workflow_attempt_capability.clone(),
            workflow_completion_result_limit: self.workflow_completion_result_limit,
            resume: None,
            lifecycle: Some("ephemeral".into()),
            agent_type: self.agent_type.clone(),
            depth: Some(self.depth),
            fork_context: self.fork_context.clone(),
        }
    }

    pub(crate) fn runtime_facts(&self, session_id: Option<&str>) -> AgentRuntimeFacts {
        AgentRuntimeFacts {
            origin: AgentOrigin::ManagedChild,
            cwd: self.cwd.clone(),
            root_cwd: self.root_cwd.clone(),
            root: self.root.clone(),
            parent: self.parent_execution.clone(),
            depth: self.depth,
            session_id: session_id.map(String::from),
            workflow_permission_causation: self.workflow_permission_causation.clone(),
            workflow_attempt_capability_digest: self
                .workflow_attempt_capability
                .as_ref()
                .map(loopal_protocol::WorkflowAttemptCapability::digest),
            workflow_completion_result_limit: self.workflow_completion_result_limit,
            spawn: self.authority.clone(),
        }
    }
}
