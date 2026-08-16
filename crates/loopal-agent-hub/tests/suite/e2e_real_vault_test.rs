use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::Hub;
use loopal_hub_vault::HubVaultService;
use loopal_ipc::Connection;
use loopal_ipc::duplex_pair;
use loopal_secret_client::{
    ExposeSecret, HUB_RPC_BUDGET, HubSecretClient, SecretClient, SecretError,
};
use loopal_vault_age::{AgeVault, DiscoveredIdentity, Recipients};
use loopal_vault_api::Vault;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

use super::secret_test_helpers::{
    ED25519_UNENCRYPTED, PUBKEY_ALICE, spawn_hub_dispatch_loop, write_key_0600,
};

struct VaultFixture {
    _tmp: tempfile::TempDir,
    cwd: std::path::PathBuf,
    identity: Arc<DiscoveredIdentity>,
}

async fn setup_real_vault(pairs: &[(&str, &str)]) -> VaultFixture {
    let tmp = tempfile::tempdir().unwrap();
    let cwd = tmp.path().to_path_buf();
    let key_path = cwd.join("id_ed25519");
    write_key_0600(&key_path, ED25519_UNENCRYPTED);
    let identity = Arc::new(loopal_vault_age::load(&key_path).unwrap());

    let vault_dir = cwd.join(".loopal").join("vaults").join("default.vault");
    std::fs::create_dir_all(&vault_dir).unwrap();
    let recipients_path = vault_dir.join("recipients");
    let mut recipients = Recipients::new();
    recipients.add_line(PUBKEY_ALICE).unwrap();
    recipients.write(&recipients_path).unwrap();

    let store_path = vault_dir.join("store.age");
    let vault = AgeVault::new(store_path, recipients_path, identity.clone());
    vault.rekey().await.unwrap();
    for (k, v) in pairs {
        vault
            .put(k, secrecy::SecretString::from((*v).to_string()))
            .await
            .expect("vault put");
    }
    VaultFixture {
        _tmp: tmp,
        cwd,
        identity,
    }
}

async fn make_real_vault_client(
    fixture: &VaultFixture,
    agent_name: &str,
    request_cwd: std::path::PathBuf,
) -> (HubSecretClient, Arc<Mutex<Hub>>) {
    let (client_t, hub_t) = duplex_pair();
    let (client_conn, _client_rx) = Connection::new(client_t).into_listening();
    let (hub_conn, hub_rx) = Connection::new(hub_t).into_listening();

    let (event_tx, _event_rx) = mpsc::channel(64);
    let hub = Arc::new(Mutex::new(Hub::with_cwd(event_tx, fixture.cwd.clone())));
    let vault = HubVaultService::with_identity(
        fixture.identity.clone(),
        Arc::new(loopal_vault_api::NoopAuditSink),
    );
    hub.lock().await.set_vault_service(Arc::new(vault));
    hub.lock()
        .await
        .spawn_registry
        .register(agent_name.into(), fixture.cwd.clone(), None);
    spawn_hub_dispatch_loop(hub.clone(), hub_conn, hub_rx, agent_name.into()).await;

    let client = HubSecretClient::new(client_conn, request_cwd, agent_name.into(), 0);
    (client, hub)
}

#[tokio::test]
async fn real_vault_get_roundtrip_returns_exact_plaintext() {
    let fx = setup_real_vault(&[("api_key", "sk-real-plaintext-value-12345")]).await;
    let (client, _hub) = make_real_vault_client(&fx, "test-agent", fx.cwd.clone()).await;
    let plaintext = tokio::time::timeout(
        Duration::from_secs(5),
        client.get("api_key", HUB_RPC_BUDGET),
    )
    .await
    .expect("IPC must not hang")
    .expect("real vault get must succeed");
    assert_eq!(
        plaintext.expose_secret(),
        "sk-real-plaintext-value-12345",
        "IPC roundtrip must preserve exact plaintext byte-for-byte"
    );
}

#[tokio::test]
async fn real_vault_list_names_returns_all_keys() {
    let fx = setup_real_vault(&[("k1", "v1"), ("k2", "v2"), ("openai_api", "sk-x")]).await;
    let (client, _hub) = make_real_vault_client(&fx, "test-agent", fx.cwd.clone()).await;
    let mut names = tokio::time::timeout(Duration::from_secs(5), client.list_names(HUB_RPC_BUDGET))
        .await
        .expect("IPC must not hang")
        .expect("real vault list_names must succeed");
    names.sort();
    assert_eq!(
        names,
        vec!["k1".to_string(), "k2".into(), "openai_api".into()]
    );
}

#[tokio::test]
async fn real_vault_missing_secret_returns_structured_not_found() {
    let fx = setup_real_vault(&[("present", "x")]).await;
    let (client, _hub) = make_real_vault_client(&fx, "test-agent", fx.cwd.clone()).await;
    let err = tokio::time::timeout(Duration::from_secs(5), client.get("absent", HUB_RPC_BUDGET))
        .await
        .expect("IPC must not hang")
        .unwrap_err();
    match err {
        SecretError::SecretNotFound(name) => assert_eq!(name, "absent"),
        other => panic!("expected SecretNotFound, got {other:?}"),
    }
}

#[tokio::test]
async fn cross_cwd_request_denied_with_structured_permission_denied() {
    // The bug this catches: if verify_caller falls back to plain-string error,
    // the client classifies as Ipc and retry_transient pummels Hub.
    let fx_a = setup_real_vault(&[("api_key", "sk-A")]).await;
    let fx_b = setup_real_vault(&[("api_key", "sk-B")]).await;
    let (client, _hub) = make_real_vault_client(&fx_a, "test-agent", fx_b.cwd.clone()).await;
    let err = tokio::time::timeout(
        Duration::from_secs(5),
        client.get("api_key", HUB_RPC_BUDGET),
    )
    .await
    .expect("IPC must not hang")
    .unwrap_err();
    assert!(
        matches!(err, SecretError::PermissionDenied),
        "cross-cwd MUST yield typed PermissionDenied; got: {err:?}"
    );
}
