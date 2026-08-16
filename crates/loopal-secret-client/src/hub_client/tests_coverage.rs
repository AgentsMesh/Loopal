use std::time::Duration;

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::SecretListNamesRequest;
use secrecy::ExposeSecret;

use crate::{HUB_RPC_BUDGET, HubSecretClient, IpcBudget, RetryPolicy, SecretClient, SecretError};

fn assert_ipc_contains(error: SecretError, expected: &str) {
    let SecretError::Ipc(message) = error else {
        panic!("expected IPC error, got {error:?}")
    };
    assert!(
        message.contains(expected),
        "unexpected IPC error: {message}"
    );
}

#[tokio::test]
async fn list_names_and_expand_passthroughs_use_the_agent_client() {
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let (client_connection, _client_rx) = Connection::new(client_transport).into_listening();
    let (server_connection, mut server_rx) = Connection::new(server_transport).into_listening();
    let responder = tokio::spawn(async move {
        let Some(Incoming::Request { id, method, params }) = server_rx.recv().await else {
            panic!("expected list-names request")
        };
        assert_eq!(method, methods::HUB_SECRET_LIST_NAMES.name);
        let request: SecretListNamesRequest = serde_json::from_value(params).unwrap();
        assert_eq!(request.cwd, "/workspace");
        server_connection
            .respond(id, serde_json::json!({"names": ["first", "second"]}))
            .await
            .unwrap();
    });
    let client = HubSecretClient::new(
        client_connection,
        std::path::PathBuf::from("/workspace"),
        "worker".into(),
        2,
    );

    assert_eq!(
        client.list_names(HUB_RPC_BUDGET).await.unwrap(),
        ["first", "second"]
    );
    assert_eq!(
        client
            .expand_author("plain", HUB_RPC_BUDGET)
            .await
            .unwrap()
            .expose_secret(),
        "plain"
    );
    assert_eq!(
        client
            .expand_wire("plain", HUB_RPC_BUDGET)
            .await
            .unwrap()
            .expose_secret(),
        "plain"
    );
    responder.await.unwrap();
}

#[tokio::test]
async fn forbidden_and_expired_budgets_fail_closed() {
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let (client_connection, _client_rx) = Connection::new(client_transport).into_listening();
    let (_server_connection, _server_rx) = Connection::new(server_transport).into_listening();
    let client = HubSecretClient::new(
        client_connection,
        std::path::PathBuf::from("/workspace"),
        "worker".into(),
        2,
    )
    .with_retry_policy(RetryPolicy::new(1, Duration::ZERO));

    assert_ipc_contains(
        client.get("key", IpcBudget::Forbidden).await.unwrap_err(),
        "Forbidden",
    );
    assert_ipc_contains(
        client.list_names(IpcBudget::Forbidden).await.unwrap_err(),
        "Forbidden",
    );
    assert_ipc_contains(
        client
            .get("key", IpcBudget::Allowed(Duration::ZERO))
            .await
            .unwrap_err(),
        "timed out",
    );
    assert_ipc_contains(
        client
            .list_names(IpcBudget::Allowed(Duration::ZERO))
            .await
            .unwrap_err(),
        "timed out",
    );
}
