use std::sync::Arc;

use loopal_protocol::QualifiedAddress;

use super::super::lifecycle_audit::{WorkflowAttemptAudit, WorkflowAuditPhase};
use super::requests::causation;
use crate::Hub;
use crate::spawn_manager::spawn_audit_test_support::Sink;
use crate::types::AgentExecutionRef;
use crate::workflow::WorkflowOwner;

#[tokio::test]
async fn records_owned_attempt_identity_without_payload_content() {
    let (events, _receiver) = tokio::sync::mpsc::channel(4);
    let sink = Arc::new(Sink::new(false));
    let mut hub = Hub::new(events);
    hub.set_protected_audit(sink.clone());
    let owner = WorkflowOwner::new("session", QualifiedAddress::local("root"));
    let causation = causation("wrun_audit", "wnode_audit", "watt_audit");
    let execution = AgentExecutionRef::local("workflow-worker", 9);

    WorkflowAttemptAudit::new(
        &hub,
        &owner,
        &causation,
        Some(&execution),
        WorkflowAuditPhase::Activate,
    )
    .unwrap()
    .append()
    .await
    .unwrap();

    let records = sink.records();
    let record = &records[0];
    assert_eq!(
        record.op,
        loopal_vault_api::ProtectedOp::WorkflowAttemptLifecycle
    );
    assert_eq!(record.subject, "watt_audit");
    assert_eq!(record.session_id.as_deref(), Some("session"));
    assert_eq!(record.agent_name.as_deref(), Some("workflow-worker"));
    assert_eq!(record.generation, Some(9));
    assert_eq!(record.workflow_run_id.as_deref(), Some("wrun_audit"));
    assert_eq!(record.workflow_node_id.as_deref(), Some("wnode_audit"));
    assert_eq!(record.workflow_attempt_id.as_deref(), Some("watt_audit"));
    assert_eq!(record.workflow_phase.as_deref(), Some("activate"));
}

#[tokio::test]
async fn missing_or_failed_audit_is_reported() {
    let (events, _receiver) = tokio::sync::mpsc::channel(4);
    let owner = WorkflowOwner::new("session", QualifiedAddress::local("root"));
    let causation = causation("wrun_audit", "wnode_audit", "watt_audit");
    let hub = Hub::new(events);
    assert!(
        WorkflowAttemptAudit::new(&hub, &owner, &causation, None, WorkflowAuditPhase::Prepare,)
            .is_err()
    );

    let (events, _receiver) = tokio::sync::mpsc::channel(4);
    let mut hub = Hub::new(events);
    hub.set_protected_audit(Arc::new(Sink::new(true)));
    let error =
        WorkflowAttemptAudit::new(&hub, &owner, &causation, None, WorkflowAuditPhase::Shutdown)
            .unwrap()
            .append()
            .await
            .unwrap_err();
    assert!(error.contains("workflow lifecycle audit failed"));
}

#[test]
fn every_lifecycle_phase_has_a_stable_name() {
    let phases = [
        WorkflowAuditPhase::Prepare,
        WorkflowAuditPhase::Activate,
        WorkflowAuditPhase::Interrupt,
        WorkflowAuditPhase::Shutdown,
    ];
    assert_eq!(phases.len(), 4);
}
