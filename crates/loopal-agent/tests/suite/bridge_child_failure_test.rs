use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use loopal_agent::bridge::bridge_child_events;
use loopal_agent_client::AgentClient;
use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentCompletion;

use crate::bridge_child_test::{make_duplex_pair, start_bridge_client};

const T: std::time::Duration = std::time::Duration::from_secs(10);

async fn scripted_completion(reason: &str, result: Option<&str>) -> Result<String, String> {
    let (server_transport, client_transport) = make_duplex_pair();
    let (server, _server_rx) = Connection::new(server_transport).into_listening();
    let client = AgentClient::new(client_transport);
    server
        .send_notification(
            methods::AGENT_COMPLETED.name,
            serde_json::to_value(AgentCompletion {
                reason: reason.into(),
                result: result.map(String::from),
            })
            .unwrap(),
        )
        .await
        .unwrap();
    let (event_tx, _event_rx) = mpsc::channel(16);
    bridge_child_events(client, &event_tx, "test", &CancellationToken::new()).await
}

#[tokio::test]
async fn terminal_child_error_is_not_empty_success() {
    let calls = vec![vec![loopal_test_support::chunks::non_retryable_error(
        "invalid child request",
    )]];
    let (client, event_tx, cancel, _fixture) = start_bridge_client(calls).await;
    let error = tokio::time::timeout(T, bridge_child_events(client, &event_tx, "test", &cancel))
        .await
        .expect("bridge should reach agent/completed")
        .expect_err("terminal Error must fail the bridge result");
    assert!(error.contains("invalid child request"));
}

#[tokio::test]
async fn every_non_goal_completion_reason_is_rejected() {
    for (reason, result) in [
        ("aborted", Some("parent stopped child")),
        ("shutdown", None),
        ("future-reason", Some("new protocol detail")),
    ] {
        let error = scripted_completion(reason, result)
            .await
            .expect_err("only goal completion may succeed");
        assert!(error.contains(reason));
        if let Some(result) = result {
            assert!(error.contains(result));
        }
    }
}

#[tokio::test]
async fn disconnect_before_completion_is_rejected() {
    let (server_transport, client_transport) = make_duplex_pair();
    let client = AgentClient::new(client_transport);
    drop(server_transport);
    let (event_tx, _event_rx) = mpsc::channel(16);
    let error = tokio::time::timeout(
        T,
        bridge_child_events(client, &event_tx, "test", &CancellationToken::new()),
    )
    .await
    .expect("bridge should observe transport EOF")
    .expect_err("disconnect must not become an empty success result");
    assert_eq!(
        error,
        "sub-agent test connection closed before agent/completed"
    );
}
