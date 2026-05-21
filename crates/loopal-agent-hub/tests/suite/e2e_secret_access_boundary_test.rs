use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::Hub;
use loopal_hub_vault::HubVaultService;
use loopal_ipc::Connection;
use loopal_ipc::duplex_pair;
use loopal_secret_client::{HubSecretClient, SecretClient, SecretError};
use tokio::sync::Mutex;
use tokio::sync::mpsc;

use super::secret_test_helpers::spawn_hub_dispatch_loop;

#[tokio::test]
async fn sub_agent_requesting_cwd_outside_spawn_tree_is_denied() {
    let (client_t, hub_t) = duplex_pair();
    let client_conn = Arc::new(Connection::new(client_t));
    let hub_conn = Arc::new(Connection::new(hub_t));
    let _client_rx = client_conn.start();

    let (event_tx, _event_rx) = mpsc::channel(64);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let vault = HubVaultService::with_noop_audit().expect("noop vault construct");

    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();
    hub.lock()
        .await
        .spawn_registry
        .register("test-agent".into(), dir_a.path().to_path_buf(), None);
    hub.lock().await.set_vault_service(Arc::new(vault));

    spawn_hub_dispatch_loop(hub.clone(), hub_conn, "test-agent".into());

    let client = HubSecretClient::new(
        client_conn,
        dir_b.path().to_path_buf(),
        "test-agent".into(),
        0,
    );

    let err = tokio::time::timeout(Duration::from_secs(5), client.get("anything"))
        .await
        .expect("IPC must not hang")
        .unwrap_err();

    assert!(
        matches!(err, SecretError::PermissionDenied),
        "cross-cwd access must be denied with structured PermissionDenied, got: {err:?}"
    );
}

#[tokio::test]
async fn agent_requesting_own_cwd_is_allowed_through_verify_caller() {
    let (client_t, hub_t) = duplex_pair();
    let client_conn = Arc::new(Connection::new(client_t));
    let hub_conn = Arc::new(Connection::new(hub_t));
    let _client_rx = client_conn.start();

    let (event_tx, _event_rx) = mpsc::channel(64);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let vault = HubVaultService::with_noop_audit().expect("noop vault");

    let dir = tempfile::tempdir().unwrap();
    hub.lock()
        .await
        .spawn_registry
        .register("self-agent".into(), dir.path().to_path_buf(), None);
    hub.lock().await.set_vault_service(Arc::new(vault));

    spawn_hub_dispatch_loop(hub.clone(), hub_conn, "self-agent".into());

    let client = HubSecretClient::new(
        client_conn,
        dir.path().to_path_buf(),
        "self-agent".into(),
        0,
    );

    let err = tokio::time::timeout(Duration::from_secs(5), client.get("x"))
        .await
        .expect("IPC must not hang")
        .unwrap_err();

    // verify_caller passed (cwd matches) → reaches vault.get → VaultNotFound,
    // not PermissionDenied — proves the boundary check is path-aware.
    assert!(
        !matches!(err, SecretError::PermissionDenied),
        "in-tree access must not be denied; got: {err:?}"
    );
}
