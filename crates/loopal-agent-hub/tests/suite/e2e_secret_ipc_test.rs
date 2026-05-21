use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::Hub;
use loopal_ipc::Connection;
use loopal_ipc::duplex_pair;
use loopal_secret_client::{HubSecretClient, SecretClient, SecretError};
use tokio::sync::Mutex;
use tokio::sync::mpsc;

use super::secret_test_helpers::spawn_hub_dispatch_loop;

fn make_client_and_hub() -> (HubSecretClient, Arc<Mutex<Hub>>) {
    let (client_t, hub_t) = duplex_pair();
    let client_conn = Arc::new(Connection::new(client_t));
    let hub_conn = Arc::new(Connection::new(hub_t));
    let _client_rx = client_conn.start();

    let (event_tx, _event_rx) = mpsc::channel(64);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    spawn_hub_dispatch_loop(hub.clone(), hub_conn, "test-client".into());

    let client = HubSecretClient::new(
        client_conn,
        PathBuf::from("/nonexistent-test-cwd"),
        "test-agent".into(),
        0,
    );
    (client, hub)
}

#[tokio::test]
async fn vault_not_initialized_returns_structured_error() {
    let (client, _hub) = make_client_and_hub();
    let result = tokio::time::timeout(Duration::from_secs(5), client.get("api_key"))
        .await
        .expect("IPC must not hang")
        .unwrap_err();
    assert!(
        matches!(result, SecretError::Ipc(_)),
        "unstructured Hub error must bucket as transient Ipc, got: {result:?}"
    );
}

#[tokio::test]
async fn list_names_no_vault_returns_structured_error() {
    let (client, _hub) = make_client_and_hub();
    let result = tokio::time::timeout(Duration::from_secs(5), client.list_names())
        .await
        .expect("IPC must not hang")
        .unwrap_err();
    assert!(matches!(result, SecretError::Ipc(_)));
}
