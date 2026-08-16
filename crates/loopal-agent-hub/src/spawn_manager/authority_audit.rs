use std::path::PathBuf;
use std::sync::Arc;

use loopal_vault_api::{AuditMetadata, AuditSink, ProtectedOp};

use super::{PreparedSpawn, SpawnRequestLease};
use crate::Hub;

#[derive(Clone)]
pub(crate) struct SpawnAudit {
    sink: Arc<dyn AuditSink>,
    subject: String,
    session_id: Option<String>,
    cwd: PathBuf,
    agent_name: Option<String>,
    depth: u32,
    connection_generation: Option<u64>,
    workflow_run_id: Option<String>,
    workflow_node_id: Option<String>,
    workflow_attempt_id: Option<String>,
    spawn_target: String,
    model: String,
    permission_mode: String,
    decision_mode: String,
    sandbox_policy: String,
}

struct SpawnAuditInput<'a> {
    subject: &'a str,
    cwd: &'a std::path::Path,
    depth: u32,
    authority: &'a crate::types::SpawnAuthority,
    spawn_target: String,
    actor: Option<(
        &'a crate::types::AgentExecutionRef,
        &'a crate::types::AgentRuntimeFacts,
    )>,
    workflow_causation: Option<&'a loopal_protocol::WorkflowPermissionCausation>,
}

impl SpawnAudit {
    pub(crate) fn for_prepared(hub: &Hub, prepared: &PreparedSpawn) -> Result<Self, String> {
        let (actor, spawn_target) = match &prepared.request_lease {
            SpawnRequestLease::Agent(execution) => (
                Some((
                    execution,
                    hub.registry.runtime_facts(execution).ok_or_else(|| {
                        "spawn requester runtime authority is unavailable".to_string()
                    })?,
                )),
                "local",
            ),
            SpawnRequestLease::TrustedMetaHub(_) => (None, "remote_destination"),
            SpawnRequestLease::Internal => {
                return Err("internal process spawn authority is unauditable".into());
            }
        };
        Self::new(
            hub,
            SpawnAuditInput {
                subject: &prepared.name,
                cwd: &prepared.cwd,
                depth: prepared.depth,
                authority: &prepared.authority,
                spawn_target: spawn_target.to_string(),
                actor,
                workflow_causation: prepared.workflow_permission_causation.as_ref(),
            },
        )
    }

    pub(crate) fn for_cross_hub(
        hub: &Hub,
        subject: &str,
        target_hub: &str,
        requester: &crate::types::AgentExecutionRef,
        params: &serde_json::Value,
    ) -> Result<Self, String> {
        let facts = hub
            .registry
            .runtime_facts(requester)
            .ok_or_else(|| "spawn requester runtime authority is unavailable".to_string())?;
        let authority = crate::types::SpawnAuthority {
            model: required_string(params, "model")?,
            permission_mode: required_string(params, "permission_mode")?
                .parse::<loopal_tool_api::PermissionMode>()
                .map_err(|error| error.to_string())?,
            decision_mode: required_string(params, "decision_mode")?
                .parse::<loopal_decision_api::DecisionMode>()
                .map_err(|error| error.to_string())?,
            sandbox_policy: required_string(params, "sandbox_policy")?
                .parse::<loopal_config::SandboxPolicy>()?,
        };
        let depth = params["depth"]
            .as_u64()
            .and_then(|depth| u32::try_from(depth).ok())
            .ok_or_else(|| "invalid derived spawn depth".to_string())?;
        Self::new(
            hub,
            SpawnAuditInput {
                subject,
                cwd: &facts.cwd,
                depth,
                authority: &authority,
                spawn_target: format!("hub:{target_hub}"),
                actor: Some((requester, facts)),
                workflow_causation: None,
            },
        )
    }

    fn new(hub: &Hub, input: SpawnAuditInput<'_>) -> Result<Self, String> {
        let sink = hub
            .protected_audit
            .clone()
            .ok_or_else(|| "protected audit unavailable".to_string())?;
        let workflow = input.workflow_causation.or_else(|| {
            input
                .actor
                .and_then(|(_, facts)| facts.workflow_permission_causation.as_ref())
        });
        Ok(Self {
            sink,
            subject: input.subject.to_string(),
            session_id: input.actor.and_then(|(_, facts)| facts.session_id.clone()),
            cwd: input.cwd.to_path_buf(),
            agent_name: input
                .actor
                .map(|(execution, _)| execution.address.agent.clone()),
            depth: input.depth,
            connection_generation: input
                .actor
                .map(|(execution, _)| execution.connection_generation),
            workflow_run_id: workflow.map(|value| value.run_id.to_string()),
            workflow_node_id: workflow.map(|value| value.node_id.to_string()),
            workflow_attempt_id: workflow.map(|value| value.attempt_id.to_string()),
            spawn_target: input.spawn_target,
            model: input.authority.model.clone(),
            permission_mode: input.authority.permission_mode.to_string(),
            decision_mode: input.authority.decision_mode.to_string(),
            sandbox_policy: input.authority.sandbox_policy.to_string(),
        })
    }

    pub(crate) async fn append(self) -> Result<(), String> {
        tokio::task::spawn_blocking(move || {
            self.sink.record_protected(
                ProtectedOp::SpawnAuthority,
                &self.subject,
                &AuditMetadata {
                    session_id: self.session_id.as_deref(),
                    cwd: Some(&self.cwd),
                    agent_name: self.agent_name.as_deref(),
                    depth: Some(self.depth),
                    connection_generation: self.connection_generation,
                    workflow_run_id: self.workflow_run_id.as_deref(),
                    workflow_node_id: self.workflow_node_id.as_deref(),
                    workflow_attempt_id: self.workflow_attempt_id.as_deref(),
                    spawn_target: Some(&self.spawn_target),
                    model: Some(&self.model),
                    permission_mode: Some(&self.permission_mode),
                    decision_mode: Some(&self.decision_mode),
                    sandbox_policy: Some(&self.sandbox_policy),
                    ..AuditMetadata::default()
                },
            )
        })
        .await
        .map_err(|error| format!("spawn authority audit task failed: {error}"))?
        .map_err(|error| format!("spawn authority audit failed: {error}"))
    }
}

fn required_string(params: &serde_json::Value, field: &str) -> Result<String, String> {
    params[field]
        .as_str()
        .map(String::from)
        .ok_or_else(|| format!("derived spawn '{field}' must be a string"))
}
