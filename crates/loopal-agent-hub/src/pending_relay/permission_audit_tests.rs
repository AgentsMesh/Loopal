use std::sync::{Arc, Mutex as StdMutex};

use loopal_ipc::Connection;
use loopal_protocol::{
    PermissionActionDigest, PermissionAuditDecision, PermissionAuditSource,
    PermissionDecisionAuditRequest, PermissionIntentDigest, PermissionSchemaDigest,
    WorkflowAttemptId, WorkflowNodeId, WorkflowPermissionCausation, WorkflowRunId,
};
use loopal_vault_api::{AuditMetadata, AuditResult, AuditSink, ProtectedOp, VaultOp};
use tokio::sync::{Mutex, mpsc};

use super::record_for_execution;
use crate::Hub;
use crate::types::{AgentRuntimeFacts, SpawnAuthority};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Captured {
    session_id: Option<String>,
    connection_generation: Option<u64>,
    intent_digest: Option<String>,
    workflow_run_id: Option<String>,
    workflow_node_id: Option<String>,
    workflow_attempt_id: Option<String>,
    decision: Option<String>,
    source: Option<String>,
}

#[derive(Default)]
struct Sink(StdMutex<Vec<Captured>>);

impl AuditSink for Sink {
    fn record(&self, _: VaultOp, _: &str, _: &AuditMetadata<'_>) -> AuditResult<()> {
        Ok(())
    }

    fn record_protected(
        &self,
        op: ProtectedOp,
        _: &str,
        metadata: &AuditMetadata<'_>,
    ) -> AuditResult<()> {
        assert_eq!(op, ProtectedOp::PermissionDecision);
        self.0.lock().unwrap().push(Captured {
            session_id: metadata.session_id.map(str::to_owned),
            connection_generation: metadata.connection_generation,
            intent_digest: metadata.intent_digest.map(str::to_owned),
            workflow_run_id: metadata.workflow_run_id.map(str::to_owned),
            workflow_node_id: metadata.workflow_node_id.map(str::to_owned),
            workflow_attempt_id: metadata.workflow_attempt_id.map(str::to_owned),
            decision: metadata.decision.map(str::to_owned),
            source: metadata.decision_source.map(str::to_owned),
        });
        Ok(())
    }
}

#[tokio::test]
async fn records_authenticated_session_generation_and_workflow() {
    let (events, _event_rx) = mpsc::channel(8);
    let mut hub = Hub::new(events);
    let (agent_transport, hub_transport) = loopal_ipc::duplex_pair();
    let (agent, _agent_rx) = Connection::new(agent_transport).into_listening();
    let (connection, _hub_rx) = Connection::new(hub_transport).into_listening();
    let execution = hub
        .registry
        .register_connection_with_parent_execution("worker", connection.clone(), None, None, None)
        .unwrap();
    let workflow = WorkflowPermissionCausation {
        run_id: WorkflowRunId::new("wrun_audit"),
        node_id: WorkflowNodeId::new("wnode_audit"),
        attempt_id: WorkflowAttemptId::new("watt_audit"),
    };
    let mut facts = AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default());
    facts.session_id = Some("session-audit".into());
    facts.workflow_permission_causation = Some(workflow);
    assert!(hub.registry.set_runtime_facts(&execution, facts));
    let sink = Arc::new(Sink::default());
    hub.set_protected_audit(sink.clone());
    let hub = Arc::new(Mutex::new(hub));

    let intent = PermissionIntentDigest::from_bytes([0x33; 32]);
    let request = PermissionDecisionAuditRequest::new(
        "call-audit",
        "Bash",
        PermissionActionDigest::from_bytes([0x11; 32]),
        PermissionSchemaDigest::from_bytes([0x22; 32]),
        Some(intent),
        PermissionAuditDecision::Allow,
        PermissionAuditSource::Ui,
    )
    .unwrap();
    record_for_execution(&hub, &execution, &connection, &request)
        .await
        .unwrap();
    drop(agent);

    assert_eq!(
        sink.0.lock().unwrap().as_slice(),
        &[Captured {
            session_id: Some("session-audit".into()),
            connection_generation: Some(execution.connection_generation),
            intent_digest: Some(intent.to_string()),
            workflow_run_id: Some("wrun_audit".into()),
            workflow_node_id: Some("wnode_audit".into()),
            workflow_attempt_id: Some("watt_audit".into()),
            decision: Some("allow".into()),
            source: Some("ui".into()),
        }]
    );
}
