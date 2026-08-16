use std::sync::Arc;

use loopal_protocol::{
    PermissionDecisionAuditRequest, PermissionDecisionAuditResponse, ProtectedEffectAuditRequest,
    ProtectedEffectAuditResponse,
};
use loopal_vault_api::{AuditMetadata, ProtectedOp};
use tokio::sync::Mutex;

use crate::hub::Hub;
use crate::request_principal::AgentPrincipal;

pub async fn handle_permission_decision(
    hub: &Arc<Mutex<Hub>>,
    params: serde_json::Value,
    agent: &AgentPrincipal,
) -> Result<serde_json::Value, String> {
    let request: PermissionDecisionAuditRequest = serde_json::from_value(params)
        .map_err(|error| format!("invalid permission decision audit request: {error}"))?;
    request
        .validate()
        .map_err(|error| format!("invalid permission decision audit request: {error}"))?;
    if matches!(
        request.source(),
        loopal_protocol::PermissionAuditSource::Ui
            | loopal_protocol::PermissionAuditSource::RememberedGrant
    ) {
        return Err("permission audit source is reserved for Hub authority".into());
    }
    let audit = {
        let locked = hub.lock().await;
        if !locked.registry.owns_active_lease(&agent.execution) {
            return Err("stale Agent connection".into());
        }
        locked
            .protected_audit
            .clone()
            .ok_or_else(|| "protected audit unavailable".to_string())?
    };
    let audit_agent = agent.clone();
    let execution = agent.execution.clone();
    let audit_execution = execution.clone();
    let subject = request.tool_call_id().to_string();
    let tool_name = request.tool_name().to_string();
    let action_digest = request.action_digest().to_string();
    let schema_digest = request.schema_digest().to_string();
    let intent_digest = request.intent_digest().map(|digest| digest.to_string());
    let decision = request.decision().as_str();
    let source = request.source().as_str();
    tokio::task::spawn_blocking(move || {
        audit.record_protected(
            ProtectedOp::PermissionDecision,
            &subject,
            &AuditMetadata {
                session_id: audit_agent.session_id.as_deref(),
                cwd: Some(&audit_agent.cwd),
                agent_name: Some(&audit_execution.address.agent),
                depth: Some(audit_agent.depth),
                connection_generation: Some(audit_execution.connection_generation),
                tool_name: Some(&tool_name),
                tool_call_id: Some(&subject),
                action_digest: Some(&action_digest),
                schema_digest: Some(&schema_digest),
                intent_digest: intent_digest.as_deref(),
                workflow_run_id: audit_agent
                    .workflow_permission_causation
                    .as_ref()
                    .map(|workflow| workflow.run_id.as_str()),
                workflow_node_id: audit_agent
                    .workflow_permission_causation
                    .as_ref()
                    .map(|workflow| workflow.node_id.as_str()),
                workflow_attempt_id: audit_agent
                    .workflow_permission_causation
                    .as_ref()
                    .map(|workflow| workflow.attempt_id.as_str()),
                decision: Some(decision),
                decision_source: Some(source),
                ..AuditMetadata::default()
            },
        )
    })
    .await
    .map_err(|error| format!("protected audit task failed: {error}"))?
    .map_err(|error| format!("protected audit failed: {error}"))?;
    if !hub.lock().await.registry.owns_active_lease(&execution) {
        return Err("stale Agent connection after protected audit".into());
    }
    serde_json::to_value(PermissionDecisionAuditResponse { recorded: true })
        .map_err(|error| error.to_string())
}

pub async fn handle_protected_effect(
    hub: &Arc<Mutex<Hub>>,
    params: serde_json::Value,
    agent: &AgentPrincipal,
) -> Result<serde_json::Value, String> {
    let request: ProtectedEffectAuditRequest = serde_json::from_value(params)
        .map_err(|error| format!("invalid protected effect audit request: {error}"))?;
    request
        .validate()
        .map_err(|error| format!("invalid protected effect audit request: {error}"))?;
    let receipt = request.receipt().cloned();
    if agent.workflow_permission_causation.is_some() && receipt.is_none() {
        return Err("workflow protected effect requires a Hub permission receipt".into());
    }
    let audit = {
        let mut locked = hub.lock().await;
        if !locked.registry.owns_active_lease(&agent.execution) {
            return Err("stale Agent connection".into());
        }
        if let Some(receipt) = receipt.as_ref() {
            let current_ui_generation = locked.ui.capability_snapshot().generation;
            locked.permission_receipts.consume(
                receipt,
                request.action_digest(),
                request.schema_digest(),
                &agent.execution,
                agent.workflow_permission_causation.as_ref(),
                current_ui_generation,
            )?;
        }
        locked
            .protected_audit
            .clone()
            .ok_or_else(|| "protected audit unavailable".to_string())?
    };
    let audit_agent = agent.clone();
    let execution = agent.execution.clone();
    let audit_execution = execution.clone();
    let subject = request.tool_call_id().to_string();
    let tool_name = request.tool_name().to_string();
    let action_digest = request.action_digest().to_string();
    let schema_digest = request.schema_digest().to_string();
    tokio::task::spawn_blocking(move || {
        audit.record_protected(
            ProtectedOp::ToolEffect,
            &subject,
            &AuditMetadata {
                session_id: audit_agent.session_id.as_deref(),
                cwd: Some(&audit_agent.cwd),
                agent_name: Some(&audit_execution.address.agent),
                depth: Some(audit_agent.depth),
                connection_generation: Some(audit_execution.connection_generation),
                tool_name: Some(&tool_name),
                tool_call_id: Some(&subject),
                action_digest: Some(&action_digest),
                schema_digest: Some(&schema_digest),
                workflow_run_id: audit_agent
                    .workflow_permission_causation
                    .as_ref()
                    .map(|workflow| workflow.run_id.as_str()),
                workflow_node_id: audit_agent
                    .workflow_permission_causation
                    .as_ref()
                    .map(|workflow| workflow.node_id.as_str()),
                workflow_attempt_id: audit_agent
                    .workflow_permission_causation
                    .as_ref()
                    .map(|workflow| workflow.attempt_id.as_str()),
                ..AuditMetadata::default()
            },
        )
    })
    .await
    .map_err(|error| format!("protected audit task failed: {error}"))?
    .map_err(|error| format!("protected audit failed: {error}"))?;

    if !hub.lock().await.registry.owns_active_lease(&execution) {
        return Err("stale Agent connection after protected audit".into());
    }
    serde_json::to_value(ProtectedEffectAuditResponse { recorded: true })
        .map_err(|error| error.to_string())
}
