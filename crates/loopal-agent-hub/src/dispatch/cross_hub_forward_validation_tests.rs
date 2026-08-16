use serde_json::json;

use super::{check_payload_and_names, forward_cross_hub_spawn};
use crate::dispatch::cross_hub_forward::tests::{hub_with_uplink, signed_spawn};

#[test]
fn agent_name_with_slash_is_rejected() {
    let error = check_payload_and_names(&json!({"name": "worker/leaf"}), "main").unwrap_err();

    assert!(error.contains("agent name 'worker/leaf' cannot contain '/'"));
}

#[test]
fn caller_name_with_slash_is_rejected() {
    let error = check_payload_and_names(&json!({"name": "worker"}), "parent/leaf").unwrap_err();

    assert!(error.contains("caller agent name 'parent/leaf' cannot contain '/'"));
}

#[tokio::test]
async fn zero_budget_rejects_before_remote_spawn() {
    let (event_tx, _event_rx) = tokio::sync::mpsc::channel(1);
    let (hub, _meta_connection, mut meta_rx, requester) = hub_with_uplink(event_tx).await;
    hub.lock().await.max_total_agents = 0;

    let error = forward_cross_hub_spawn(&hub, signed_spawn("worker"), &requester)
        .await
        .unwrap_err();

    assert!(error.contains("Spawn budget exhausted (0/0 sub-agents)"));
    assert!(hub.lock().await.registry.agent_info("worker").is_none());
    assert!(meta_rx.try_recv().is_err());
}
