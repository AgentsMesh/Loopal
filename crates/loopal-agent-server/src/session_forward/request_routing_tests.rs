use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_protocol::{Envelope, MessageSource};
use loopal_runtime::agent_input::AgentInput;

use super::forwarding_test_support::{peers, session};
use super::route_request;

#[tokio::test]
async fn accepts_message_rejects_malformed_and_returns_empty_snapshot() {
    let ((server, mut server_rx), (client, _client_rx)) = peers();
    let (session, mut input_rx) = session("router-session", 1);

    let message = tokio::spawn({
        let client = client.clone();
        async move {
            client
                .send_request(
                    methods::AGENT_MESSAGE.name,
                    serde_json::to_value(Envelope::new(
                        MessageSource::Human,
                        "main",
                        "request message",
                    ))
                    .unwrap(),
                )
                .await
        }
    });
    let Incoming::Request { id, method, params } = server_rx.recv().await.unwrap() else {
        panic!("expected message request")
    };
    route_request(id, &method, params, &session, &server).await;
    assert_eq!(message.await.unwrap().unwrap()["ok"], true);
    assert!(matches!(
        input_rx.recv().await,
        Some(AgentInput::Message(_))
    ));

    let malformed = tokio::spawn({
        let client = client.clone();
        async move {
            client
                .send_request(
                    methods::AGENT_MESSAGE.name,
                    serde_json::json!({"invalid": true}),
                )
                .await
        }
    });
    let Incoming::Request { id, method, params } = server_rx.recv().await.unwrap() else {
        panic!("expected malformed message request")
    };
    route_request(id, &method, params, &session, &server).await;
    assert_eq!(
        malformed.await.unwrap().unwrap_err().remote_code(),
        Some(loopal_ipc::jsonrpc::INVALID_REQUEST)
    );

    let snapshot = tokio::spawn({
        let client = client.clone();
        async move {
            client
                .send_request(methods::AGENT_STATE_SNAPSHOT.name, serde_json::json!({}))
                .await
        }
    });
    let Incoming::Request { id, method, params } = server_rx.recv().await.unwrap() else {
        panic!("expected snapshot request")
    };
    route_request(id, &method, params, &session, &server).await;
    let empty = serde_json::to_value(loopal_protocol::AgentStateSnapshot::empty()).unwrap();
    assert_eq!(snapshot.await.unwrap().unwrap(), empty);
}
