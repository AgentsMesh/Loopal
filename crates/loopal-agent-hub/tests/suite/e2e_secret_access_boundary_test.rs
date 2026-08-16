use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::Hub;
use loopal_hub_vault::HubVaultService;
use loopal_ipc::Connection;
use loopal_ipc::duplex_pair;
use loopal_secret_client::{HUB_RPC_BUDGET, HubSecretClient, SecretClient, SecretError};
use tokio::sync::Mutex;
use tokio::sync::mpsc;

use super::secret_test_helpers::{ED25519_UNENCRYPTED, spawn_hub_dispatch_loop, write_key_0600};

fn test_identity_in(dir: &std::path::Path) -> Arc<loopal_vault_age::DiscoveredIdentity> {
    let key_path = dir.join("id_ed25519");
    write_key_0600(&key_path, ED25519_UNENCRYPTED);
    Arc::new(loopal_vault_age::load(&key_path).unwrap())
}

#[tokio::test]
async fn sub_agent_requesting_cwd_outside_spawn_tree_is_denied() {
    let (client_t, hub_t) = duplex_pair();
    let (client_conn, _client_rx) = Connection::new(client_t).into_listening();
    let (hub_conn, hub_rx) = Connection::new(hub_t).into_listening();

    let (event_tx, _event_rx) = mpsc::channel(64);
    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();
    let hub = Arc::new(Mutex::new(Hub::with_cwd(
        event_tx,
        dir_a.path().to_path_buf(),
    )));
    let identity = test_identity_in(dir_a.path());
    let vault = HubVaultService::with_identity(identity, Arc::new(loopal_vault_api::NoopAuditSink));

    hub.lock().await.set_vault_service(Arc::new(vault));

    spawn_hub_dispatch_loop(hub.clone(), hub_conn, hub_rx, "test-agent".into()).await;

    let client = HubSecretClient::new(
        client_conn,
        dir_b.path().to_path_buf(),
        "test-agent".into(),
        0,
    );

    let err = tokio::time::timeout(
        Duration::from_secs(5),
        client.get("anything", HUB_RPC_BUDGET),
    )
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
    let (client_conn, _client_rx) = Connection::new(client_t).into_listening();
    let (hub_conn, hub_rx) = Connection::new(hub_t).into_listening();

    let (event_tx, _event_rx) = mpsc::channel(64);
    let dir = tempfile::tempdir().unwrap();
    let hub = Arc::new(Mutex::new(Hub::with_cwd(
        event_tx,
        dir.path().to_path_buf(),
    )));
    let identity = test_identity_in(dir.path());
    let vault = HubVaultService::with_identity(identity, Arc::new(loopal_vault_api::NoopAuditSink));

    hub.lock().await.set_vault_service(Arc::new(vault));

    spawn_hub_dispatch_loop(hub.clone(), hub_conn, hub_rx, "self-agent".into()).await;

    let client = HubSecretClient::new(
        client_conn,
        dir.path().to_path_buf(),
        "self-agent".into(),
        0,
    );

    let err = tokio::time::timeout(Duration::from_secs(5), client.get("x", HUB_RPC_BUDGET))
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
