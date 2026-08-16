use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::PermissionDecisionAuditRequest;
use loopal_vault_api::{AuditMetadata, ProtectedOp};
use tokio::sync::Mutex;

use crate::hub::Hub;
use crate::types::AgentExecutionRef;

#[cfg(test)]
#[path = "permission_audit_tests.rs"]
mod tests;

pub(super) async fn record_for_execution(
    hub: &Arc<Mutex<Hub>>,
    execution: &AgentExecutionRef,
    connection: &Arc<Connection<Listening>>,
    request: &PermissionDecisionAuditRequest,
) -> Result<(), String> {
    request
        .validate()
        .map_err(|error| format!("invalid permission decision audit request: {error}"))?;
    let (audit, facts) = {
        let locked = hub.lock().await;
        if !is_current(&locked, execution, connection) {
            return Err("stale Agent permission lease".into());
        }
        let facts = locked
            .registry
            .runtime_facts(execution)
            .cloned()
            .ok_or_else(|| "Agent permission authority is unavailable".to_string())?;
        let audit = locked
            .protected_audit
            .clone()
            .ok_or_else(|| "protected audit unavailable".to_string())?;
        (audit, facts)
    };
    let request = request.clone();
    let execution = execution.clone();
    let audit_execution = execution.clone();
    tokio::task::spawn_blocking(move || {
        let action_digest = request.action_digest().to_string();
        let schema_digest = request.schema_digest().to_string();
        let intent_digest = request.intent_digest().map(|digest| digest.to_string());
        audit.record_protected(
            ProtectedOp::PermissionDecision,
            request.tool_call_id(),
            &AuditMetadata {
                session_id: facts.session_id.as_deref(),
                cwd: Some(&facts.cwd),
                agent_name: Some(&audit_execution.address.agent),
                depth: Some(facts.depth),
                connection_generation: Some(audit_execution.connection_generation),
                tool_name: Some(request.tool_name()),
                tool_call_id: Some(request.tool_call_id()),
                action_digest: Some(&action_digest),
                schema_digest: Some(&schema_digest),
                intent_digest: intent_digest.as_deref(),
                workflow_run_id: facts
                    .workflow_permission_causation
                    .as_ref()
                    .map(|workflow| workflow.run_id.as_str()),
                workflow_node_id: facts
                    .workflow_permission_causation
                    .as_ref()
                    .map(|workflow| workflow.node_id.as_str()),
                workflow_attempt_id: facts
                    .workflow_permission_causation
                    .as_ref()
                    .map(|workflow| workflow.attempt_id.as_str()),
                decision: Some(request.decision().as_str()),
                decision_source: Some(request.source().as_str()),
                ..AuditMetadata::default()
            },
        )
    })
    .await
    .map_err(|error| format!("protected audit task failed: {error}"))?
    .map_err(|error| format!("protected audit failed: {error}"))?;

    if !is_current(&*hub.lock().await, &execution, connection) {
        return Err("stale Agent permission lease after protected audit".into());
    }
    Ok(())
}

fn is_current(
    hub: &Hub,
    execution: &AgentExecutionRef,
    connection: &Arc<Connection<Listening>>,
) -> bool {
    hub.registry.owns_active_lease(execution)
        && hub
            .registry
            .exact_connection(execution)
            .is_some_and(|active| Arc::ptr_eq(&active, connection))
}
