use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_protocol::{
    PermissionActionDigest, PermissionAuditDecision, PermissionAuditSource,
    PermissionDecisionAuditRequest, PermissionIntentDigest, PermissionSchemaDigest,
};
use loopal_tool_api::ProtectedEffectAudit;
use serde_json::json;

use super::HubProtectedEffectAudit;

fn request() -> PermissionDecisionAuditRequest {
    PermissionDecisionAuditRequest::new(
        "call-1",
        "Bash",
        PermissionActionDigest::from_bytes([0x11; 32]),
        PermissionSchemaDigest::from_bytes([0x22; 32]),
        Some(PermissionIntentDigest::from_bytes([0x33; 32])),
        PermissionAuditDecision::Allow,
        PermissionAuditSource::Policy,
    )
    .unwrap()
}

fn pair() -> (
    HubProtectedEffectAudit,
    std::sync::Arc<Connection<Listening>>,
    tokio::sync::mpsc::Receiver<Incoming>,
) {
    let (client_transport, hub_transport) = loopal_ipc::duplex_pair();
    let (client, _client_rx) = Connection::new(client_transport).into_listening();
    let (hub, hub_rx) = Connection::new(hub_transport).into_listening();
    (HubProtectedEffectAudit::new(client), hub, hub_rx)
}

async fn pending_request(response: serde_json::Value) -> loopal_error::Result<()> {
    let (audit, hub, mut incoming) = pair();
    let task = tokio::spawn(async move { audit.record_permission_decision(&request()).await });
    let Incoming::Request { id, method, params } = incoming.recv().await.unwrap() else {
        panic!("expected permission audit request");
    };
    assert_eq!(
        method,
        loopal_ipc::protocol::methods::HUB_AUDIT_PERMISSION_DECISION.name
    );
    assert_eq!(
        params,
        json!({
            "tool_call_id": "call-1",
            "tool_name": "Bash",
            "action_digest": format!("sha256:{}", "11".repeat(32)),
            "schema_digest": format!("sha256:{}", "22".repeat(32)),
            "intent_digest": format!("sha256:{}", "33".repeat(32)),
            "decision": "allow",
            "source": "policy",
        })
    );
    let decoded: PermissionDecisionAuditRequest = serde_json::from_value(params).unwrap();
    assert_eq!(decoded, request());
    hub.respond(id, response).await.unwrap();
    task.await.unwrap()
}

#[tokio::test]
async fn true_ack_records_exact_typed_permission_request() {
    pending_request(json!({"recorded": true})).await.unwrap();
}

#[tokio::test]
async fn false_and_malformed_acks_fail_closed() {
    let error = pending_request(json!({"recorded": false}))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("was not recorded"));

    let error = pending_request(json!({"recorded": "yes"}))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("decode failed"));
}

#[tokio::test]
async fn invalid_permission_request_is_rejected_before_rpc() {
    let (audit, _hub, mut incoming) = pair();
    let mut value = serde_json::to_value(request()).unwrap();
    value["tool_call_id"] = json!("");
    let invalid: PermissionDecisionAuditRequest = serde_json::from_value(value).unwrap();
    let error = audit
        .record_permission_decision(&invalid)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("must be non-empty"));
    assert!(incoming.try_recv().is_err());
}
