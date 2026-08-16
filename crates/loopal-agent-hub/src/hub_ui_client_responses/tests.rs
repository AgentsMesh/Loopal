use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_ipc::{Connection, Transport, connection::Incoming};
use loopal_protocol::PermissionIntentDigest;

use crate::HubClient;

struct BlackholeTransport;

#[async_trait]
impl Transport for BlackholeTransport {
    async fn send(&self, _data: &[u8]) -> Result<(), loopal_error::LoopalError> {
        Ok(())
    }

    async fn recv(&self) -> Result<Option<Vec<u8>>, loopal_error::LoopalError> {
        std::future::pending().await
    }

    fn is_connected(&self) -> bool {
        true
    }

    async fn close(&self) {}
}

async fn capture(
    invoke: impl FnOnce(HubClient) -> tokio::task::JoinHandle<()>,
) -> (String, serde_json::Value) {
    let (client_transport, hub_transport) = loopal_ipc::duplex_pair();
    let (client, _client_rx) = Connection::new(client_transport).into_listening();
    let (hub, mut hub_rx) = Connection::new(hub_transport).into_listening();
    let task = invoke(HubClient::new(client));
    let Incoming::Request { id, method, params } = hub_rx.recv().await.unwrap() else {
        panic!("expected interaction request")
    };
    hub.respond(id, serde_json::json!({})).await.unwrap();
    task.await.unwrap();
    (method, params)
}

#[tokio::test]
async fn interaction_response_wires_carry_complete_authority() {
    let digest = PermissionIntentDigest::from_bytes([7; 32]);
    let (method, permission) = capture(|client| {
        tokio::spawn(async move {
            client
                .respond_permission_with_memory("main", "permission", Some(digest), true, true)
                .await;
        })
    })
    .await;
    assert_eq!(method, "hub/permission_response");
    assert_eq!(
        permission["permission_intent_digest"],
        serde_json::json!(digest)
    );
    assert_eq!(permission["remember_session"], true);

    let (method, question) = capture(|client| {
        tokio::spawn(async move {
            client
                .respond_question("main", "question", vec!["answer".into()])
                .await;
        })
    })
    .await;
    assert_eq!(method, "hub/question_response");
    assert_eq!(question["response"]["answers"][0], "answer");

    let (method, cancelled) = capture(|client| {
        tokio::spawn(async move { client.cancel_question("main", "question").await })
    })
    .await;
    assert_eq!(method, "hub/question_response");
    assert_eq!(cancelled["response"]["kind"], "cancelled");

    let (method, plan) = capture(|client| {
        tokio::spawn(async move {
            client
                .respond_plan_approval("main", "plan", "approve", Some("edited"))
                .await;
        })
    })
    .await;
    assert_eq!(method, "hub/plan_approval_response");
    assert_eq!(plan["edited_plan"], "edited");
}

#[tokio::test]
async fn interaction_response_does_not_wait_forever_for_transport_ack() {
    let transport: Arc<dyn Transport> = Arc::new(BlackholeTransport);
    let (connection, _incoming) = Connection::new(transport).into_listening();
    let client = HubClient::new(connection);

    tokio::time::timeout(
        Duration::from_millis(500),
        client.respond_permission("main", "interaction-token", None, true),
    )
    .await
    .expect("UI interaction response must return after its bounded deadline");
}
