use std::sync::Arc;

use loopal_protocol::WorkflowPermissionCausation;
use loopal_vault_api::{AuditMetadata, AuditSink, ProtectedOp};

use crate::Hub;
use crate::types::AgentExecutionRef;
use crate::workflow::WorkflowOwner;

use super::ProductionWorkflowSpawner;

#[derive(Clone, Copy)]
pub(super) enum WorkflowAuditPhase {
    Prepare,
    Activate,
    Interrupt,
    Shutdown,
}

impl WorkflowAuditPhase {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Prepare => "prepare",
            Self::Activate => "activate",
            Self::Interrupt => "interrupt",
            Self::Shutdown => "shutdown",
        }
    }
}

pub(super) struct WorkflowAttemptAudit {
    sink: Arc<dyn AuditSink>,
    session_id: String,
    agent_name: String,
    connection_generation: Option<u64>,
    causation: WorkflowPermissionCausation,
    phase: WorkflowAuditPhase,
}

impl WorkflowAttemptAudit {
    pub(super) fn new(
        hub: &Hub,
        owner: &WorkflowOwner,
        causation: &WorkflowPermissionCausation,
        execution: Option<&AgentExecutionRef>,
        phase: WorkflowAuditPhase,
    ) -> Result<Self, String> {
        let sink = hub
            .protected_audit
            .clone()
            .ok_or_else(|| "workflow protected audit unavailable".to_string())?;
        Ok(Self {
            sink,
            session_id: owner.session_id.clone(),
            agent_name: execution
                .map(|execution| execution.address.agent.clone())
                .unwrap_or_else(|| owner.root_agent.agent.clone()),
            connection_generation: execution.map(|execution| execution.connection_generation),
            causation: causation.clone(),
            phase,
        })
    }

    pub(super) async fn append(self) -> Result<(), String> {
        tokio::task::spawn_blocking(move || {
            self.sink.record_protected(
                ProtectedOp::WorkflowAttemptLifecycle,
                self.causation.attempt_id.as_str(),
                &AuditMetadata {
                    session_id: Some(&self.session_id),
                    agent_name: Some(&self.agent_name),
                    connection_generation: self.connection_generation,
                    workflow_run_id: Some(self.causation.run_id.as_str()),
                    workflow_node_id: Some(self.causation.node_id.as_str()),
                    workflow_attempt_id: Some(self.causation.attempt_id.as_str()),
                    workflow_phase: Some(self.phase.as_str()),
                    ..AuditMetadata::default()
                },
            )
        })
        .await
        .map_err(|error| format!("workflow lifecycle audit task failed: {error}"))?
        .map_err(|error| format!("workflow lifecycle audit failed: {error}"))
    }
}

pub(super) async fn append(
    spawner: &ProductionWorkflowSpawner,
    owner: &WorkflowOwner,
    causation: &WorkflowPermissionCausation,
    execution: Option<&AgentExecutionRef>,
    phase: WorkflowAuditPhase,
) -> Result<(), String> {
    let audit = {
        let hub = spawner.hub.lock().await;
        WorkflowAttemptAudit::new(&hub, owner, causation, execution, phase)?
    };
    audit.append().await
}

pub(super) async fn append_before_cleanup(
    spawner: &ProductionWorkflowSpawner,
    owner: &WorkflowOwner,
    causation: &WorkflowPermissionCausation,
    execution: Option<&AgentExecutionRef>,
    phase: WorkflowAuditPhase,
) {
    if let Err(error) = append(spawner, owner, causation, execution, phase).await {
        tracing::error!(
            %error,
            workflow_run_id = %causation.run_id,
            workflow_node_id = %causation.node_id,
            workflow_attempt_id = %causation.attempt_id,
            workflow_phase = phase.as_str(),
            "workflow cleanup lifecycle audit failed; cleanup will continue"
        );
    }
}
