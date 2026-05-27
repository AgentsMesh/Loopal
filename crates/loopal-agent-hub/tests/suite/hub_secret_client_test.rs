//! HubSecretClient behaviors that aren't covered by `e2e_real_vault_test` or
//! `e2e_secret_ipc_test`: IpcBudget gating, HubHealth state transitions, and
//! retry-policy interaction.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::Hub;
use loopal_ipc::{Connection, IpcBudget, duplex_pair};
use loopal_secret_client::{HUB_RPC_BUDGET, HubSecretClient, SecretClient, SecretError};
use tokio::sync::Mutex;
use tokio::sync::mpsc;

use super::secret_test_helpers::spawn_hub_dispatch_loop;

fn make_client_and_hub() -> (HubSecretClient, Arc<Mutex<Hub>>) {
    let (client_t, hub_t) = duplex_pair();
    let (client_conn, _client_rx) = Connection::new(client_t).into_listening();
    let (hub_conn, hub_rx) = Connection::new(hub_t).into_listening();

    let (event_tx, _event_rx) = mpsc::channel(64);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    spawn_hub_dispatch_loop(hub.clone(), hub_conn, hub_rx, "test-client".into());

    let client = HubSecretClient::new(
        client_conn,
        PathBuf::from("/nonexistent-test-cwd"),
        "test-agent".into(),
        0,
    );
    (client, hub)
}

#[tokio::test]
async fn forbidden_budget_rejects_get_synchronously_without_ipc() {
    // IpcBudget::Forbidden marks critical paths that must NOT issue hub RPC.
    // The client must reject these synchronously with a clear error rather
    // than letting the call go through and time out.
    let (client, _hub) = make_client_and_hub();
    let result = client
        .get("api_key", IpcBudget::Forbidden)
        .await
        .unwrap_err();
    let SecretError::Ipc(msg) = &result else {
        panic!("expected Ipc error for Forbidden budget, got: {result:?}");
    };
    assert!(
        msg.contains("Forbidden") || msg.contains("forbidden"),
        "error must reference Forbidden, got: {msg:?}"
    );
}

#[tokio::test]
async fn forbidden_budget_rejects_list_names_synchronously() {
    let (client, _hub) = make_client_and_hub();
    let result = client.list_names(IpcBudget::Forbidden).await.unwrap_err();
    let SecretError::Ipc(msg) = &result else {
        panic!("expected Ipc error, got: {result:?}");
    };
    assert!(msg.contains("Forbidden") || msg.contains("forbidden"));
}

#[tokio::test]
async fn health_starts_in_healthy_state() {
    let (client, _hub) = make_client_and_hub();
    // The `health` accessor returns the inner Arc; pre-IPC it must be
    // healthy (no failures recorded yet).
    let health = client.health();
    assert!(
        !health.is_degraded(),
        "fresh HubSecretClient must start healthy, not degraded"
    );
    assert!(
        health.degraded_at_unix_ms().is_none(),
        "no degradation timestamp on a brand-new client"
    );
}

#[tokio::test]
async fn health_degrades_after_consecutive_failures() {
    // Hub side is wired but the cwd points at a directory with no vault →
    // every call returns an Ipc/NotFound error. Enough consecutive failures
    // must flip the health to degraded.
    let (client, _hub) = make_client_and_hub();
    let health = client.health();
    assert!(!health.is_degraded());

    // Fire enough requests to cross the degradation threshold (default 3).
    for _ in 0..5 {
        let _ = tokio::time::timeout(
            Duration::from_secs(3),
            client.get("missing", HUB_RPC_BUDGET),
        )
        .await
        .expect("each get must return within budget");
    }

    assert!(
        health.is_degraded(),
        "health must transition to degraded after consecutive failures"
    );
    assert!(
        health.degraded_at_unix_ms().is_some(),
        "degraded state must carry a timestamp"
    );
}

#[tokio::test]
async fn health_observer_is_shared_across_clones() {
    // health() returns Arc<HubHealth>; clones must observe the same state
    // — degradation seen by one clone must be visible to others. This is
    // load-bearing for the agent-server settle-poll listener.
    let (client, _hub) = make_client_and_hub();
    let h1 = client.health();
    let h2 = client.health();
    assert!(Arc::ptr_eq(&h1, &h2), "health() must return the same Arc");
}
