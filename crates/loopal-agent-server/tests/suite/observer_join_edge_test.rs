//! Edge case tests for observer join behavior.

use std::sync::Arc;

use loopal_ipc::connection::Connection;

use super::bridge_helpers::make_duplex_pair;
use super::observer_join_test::init_client;

/// agent/join with no active session returns an error.
#[tokio::test]
async fn join_fails_when_no_active_session() {
    let hub = Arc::new(loopal_agent_server::session_hub::SessionHub::new());
    let (server_t, client_t) = make_duplex_pair();

    let h = hub.clone();
    tokio::spawn(async move {
        let _ = loopal_agent_server::run_test_connection(server_t, h).await;
    });

    let client = Arc::new(Connection::new(client_t));
    let _rx = client.start();
    init_client(&client).await;

    let t = std::time::Duration::from_secs(10);
    let result = tokio::time::timeout(
        t,
        client.send_request(
            loopal_ipc::protocol::methods::AGENT_JOIN.name,
            serde_json::json!({}),
        ),
    )
    .await
    .unwrap();

    if let Ok(val) = result {
        assert_ne!(
            val.get("ok"),
            Some(&serde_json::json!(true)),
            "should not succeed with no active session"
        );
    }
    // Err(_) -> RPC error, also expected
}
