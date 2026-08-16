use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_protocol::{
    PermissionActionDigest, PermissionSchemaDigest, ProtectedEffectAuditRequest,
};
use loopal_tool_api::ProtectedEffectAudit;
use serde_json::json;

use super::{AUDIT_TIMEOUT, HubProtectedEffectAudit, encode_request};

fn request() -> ProtectedEffectAuditRequest {
    ProtectedEffectAuditRequest::new(
        "call-1",
        "Bash",
        PermissionActionDigest::from_bytes([0x11; 32]),
        PermissionSchemaDigest::from_bytes([0x22; 32]),
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
    let task = tokio::spawn(async move { audit.record(&request()).await });
    let Incoming::Request { id, method, params } = incoming.recv().await.unwrap() else {
        panic!("expected audit request");
    };
    assert_eq!(
        method,
        loopal_ipc::protocol::methods::HUB_AUDIT_PROTECTED_EFFECT.name
    );
    assert_eq!(
        params,
        json!({
            "tool_call_id": "call-1",
            "tool_name": "Bash",
            "action_digest": format!("sha256:{}", "11".repeat(32)),
            "schema_digest": format!("sha256:{}", "22".repeat(32)),
        })
    );
    let decoded: ProtectedEffectAuditRequest = serde_json::from_value(params).unwrap();
    assert_eq!(decoded, request());
    hub.respond(id, response).await.unwrap();
    task.await.unwrap()
}

#[test]
fn request_encoding_has_exact_wire_shape() {
    assert_eq!(
        encode_request(&request()).unwrap(),
        json!({
            "tool_call_id": "call-1",
            "tool_name": "Bash",
            "action_digest": format!("sha256:{}", "11".repeat(32)),
            "schema_digest": format!("sha256:{}", "22".repeat(32)),
        })
    );
}

#[tokio::test]
async fn true_ack_records_exact_typed_request() {
    pending_request(json!({"recorded": true})).await.unwrap();
}

#[tokio::test]
async fn false_ack_fails_closed() {
    let error = pending_request(json!({"recorded": false}))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("was not recorded"));
}

#[tokio::test]
async fn malformed_ack_fails_closed() {
    let error = pending_request(json!({"recorded": "yes"}))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("decode failed"));
}

#[tokio::test]
async fn rpc_error_fails_closed() {
    let (audit, hub, mut incoming) = pair();
    let task = tokio::spawn(async move { audit.record(&request()).await });
    let Incoming::Request { id, .. } = incoming.recv().await.unwrap() else {
        panic!("expected audit request");
    };
    hub.respond_error(id, -32603, "audit unavailable")
        .await
        .unwrap();
    let error = task.await.unwrap().unwrap_err();
    assert!(error.to_string().contains("audit RPC failed"));
}

#[tokio::test(start_paused = true)]
async fn missing_ack_times_out_without_wall_clock_sleep() {
    let (audit, _hub, mut incoming) = pair();
    let task = tokio::spawn(async move { audit.record(&request()).await });
    assert!(matches!(
        incoming.recv().await,
        Some(Incoming::Request { .. })
    ));
    tokio::time::advance(AUDIT_TIMEOUT).await;
    let error = task.await.unwrap().unwrap_err();
    assert!(error.to_string().contains("audit timed out"));
}

#[tokio::test]
async fn invalid_request_is_rejected_before_rpc() {
    let (audit, _hub, mut incoming) = pair();
    let mut value = serde_json::to_value(request()).unwrap();
    value["tool_call_id"] = json!("");
    let invalid: ProtectedEffectAuditRequest = serde_json::from_value(value).unwrap();
    let error = audit.record(&invalid).await.unwrap_err();
    assert!(error.to_string().contains("must be non-empty"));
    assert!(incoming.try_recv().is_err());
}
