use std::sync::Arc;

use loopal_agent_hub::hub_server;
use loopal_ipc::protocol::methods;
use loopal_protocol::{
    PermissionActionDigest, PermissionAuditDecision, PermissionAuditSource,
    PermissionDecisionAuditRequest, PermissionIntentDigest, PermissionSchemaDigest,
};
use loopal_vault_api::{AuditSink, ProtectedOp};

use crate::protected_audit_support::{CapturingSink, connected, wait_registered};

fn request(source: PermissionAuditSource) -> PermissionDecisionAuditRequest {
    PermissionDecisionAuditRequest::new(
        "permission-1",
        "Bash",
        PermissionActionDigest::from_bytes([0x11; 32]),
        PermissionSchemaDigest::from_bytes([0x22; 32]),
        Some(PermissionIntentDigest::from_bytes([0x33; 32])),
        PermissionAuditDecision::Allow,
        source,
    )
    .unwrap()
}

async fn send(
    agent: &loopal_ipc::Connection<loopal_ipc::Listening>,
    request: PermissionDecisionAuditRequest,
) -> Result<serde_json::Value, loopal_ipc::RpcError> {
    agent
        .send_request(
            methods::HUB_AUDIT_PERMISSION_DECISION.name,
            serde_json::to_value(request).unwrap(),
        )
        .await
}

#[tokio::test]
async fn records_authenticated_metadata_and_digest_fields() {
    let sink = Arc::new(CapturingSink::new(false));
    let fixture = connected(Some(sink.clone())).await;

    let expected = request(PermissionAuditSource::Policy);
    let response = send(&fixture.agent, expected.clone()).await.unwrap();
    assert_eq!(response, serde_json::json!({"recorded": true}));

    let records = sink.records();
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.op, ProtectedOp::PermissionDecision);
    assert_eq!(record.subject, "permission-1");
    assert_eq!(record.session_id, None);
    assert_eq!(record.agent_name.as_deref(), Some("worker"));
    assert_eq!(record.depth, Some(0));
    assert!(record.connection_generation.is_some());
    assert_eq!(record.tool_name.as_deref(), Some("Bash"));
    assert_eq!(record.tool_call_id.as_deref(), Some("permission-1"));
    assert_eq!(
        record.action_digest.as_deref(),
        Some(expected.action_digest().to_string().as_str())
    );
    assert_eq!(
        record.schema_digest.as_deref(),
        Some(expected.schema_digest().to_string().as_str())
    );
    assert_eq!(
        record.intent_digest.as_deref(),
        Some(expected.intent_digest().unwrap().to_string().as_str())
    );
    assert_eq!(record.workflow_run_id, None);
    assert_eq!(record.workflow_node_id, None);
    assert_eq!(record.workflow_attempt_id, None);
    assert_eq!(record.decision.as_deref(), Some("allow"));
    assert_eq!(record.decision_source.as_deref(), Some("policy"));
}

#[tokio::test]
async fn agent_cannot_claim_hub_owned_sources() {
    let sink = Arc::new(CapturingSink::new(false));
    let fixture = connected(Some(sink.clone())).await;
    for source in [
        PermissionAuditSource::Ui,
        PermissionAuditSource::RememberedGrant,
    ] {
        let error = send(&fixture.agent, request(source)).await.unwrap_err();
        assert!(error.to_string().contains("reserved for Hub authority"));
    }
    assert!(sink.records().is_empty());
}

#[tokio::test]
async fn malformed_missing_and_failing_audits_are_rejected() {
    let sink = Arc::new(CapturingSink::new(false));
    let fixture = connected(Some(sink.clone())).await;
    let mut malformed = serde_json::to_value(request(PermissionAuditSource::Frontend)).unwrap();
    malformed["action_input"] = serde_json::json!({"secret": "forbidden"});
    let error = fixture
        .agent
        .send_request(methods::HUB_AUDIT_PERMISSION_DECISION.name, malformed)
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("invalid permission decision audit request")
    );
    assert!(sink.records().is_empty());

    let missing = connected(None).await;
    let error = send(&missing.agent, request(PermissionAuditSource::Policy))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("protected audit unavailable"));

    let failing_sink = Arc::new(CapturingSink::new(true));
    let failing = connected(Some(failing_sink.clone())).await;
    let error = send(&failing.agent, request(PermissionAuditSource::Policy))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("protected audit failed"));
    assert_eq!(failing_sink.records().len(), 1);
}

#[tokio::test]
async fn generation_change_after_append_denies_ack() {
    let (sink, gate) = CapturingSink::gated();
    let sink = Arc::new(sink);
    let fixture = connected(Some(sink.clone() as Arc<dyn AuditSink>)).await;
    let old_agent = fixture.agent.clone();
    let pending =
        tokio::spawn(async move { send(&old_agent, request(PermissionAuditSource::Policy)).await });
    gate.wait_started().await;

    fixture
        .hub
        .lock()
        .await
        .registry
        .unregister_connection("worker");
    let (_new_agent, _incoming) = hub_server::connect_local(fixture.hub.clone(), "worker");
    wait_registered(&fixture.hub, "worker").await;
    gate.release();

    let error = pending.await.unwrap().unwrap_err();
    assert!(
        error
            .to_string()
            .contains("stale Agent connection after protected audit")
    );
    assert_eq!(sink.records().len(), 1);
}
