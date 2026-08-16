use std::sync::Arc;

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_meta_hub::MetaHub;
use loopal_meta_hub::dispatch::dispatch_meta_request;
use serde_json::{Value, json};
use tokio::sync::Mutex;

fn spawn_payload(target_hub: &str) -> Value {
    json!({
        "name": "child",
        "model": "test-model",
        "parent": "hub-a/parent",
        "depth": 1,
        "permission_mode": "ask_dangerous",
        "decision_mode": "manual",
        "sandbox_policy": "default_write",
        "no_sandbox": false,
        "prompt": "test",
        "target_hub": target_hub,
    })
}

async fn spawn(params: Value) -> Result<Value, String> {
    dispatch_meta_request(
        &Arc::new(Mutex::new(MetaHub::new())),
        "meta/spawn",
        params,
        "hub-a".into(),
    )
    .await
}

#[tokio::test]
async fn meta_spawn_rejects_filesystem_coupled_fields() {
    for field in ["cwd", "fork_context", "resume"] {
        let mut params = spawn_payload("hub-b");
        params[field] = json!("forbidden");
        let error = spawn(params).await.expect_err("forbidden field must fail");
        assert!(error.contains(field), "got: {error}");
    }
}

#[tokio::test]
async fn meta_spawn_rejects_when_target_hub_missing() {
    let mut params = spawn_payload("hub-b");
    params.as_object_mut().unwrap().remove("target_hub");
    let error = spawn(params)
        .await
        .expect_err("missing target_hub must fail");
    assert!(error.contains("target_hub"), "got: {error}");
}

#[tokio::test]
async fn meta_spawn_rejects_missing_required_forwarded_field_before_lookup() {
    let mut params = spawn_payload("no-such-hub");
    params.as_object_mut().unwrap().remove("model");
    let error = spawn(params).await.expect_err("missing model must fail");
    assert!(error.contains("model"), "got: {error}");
    assert!(!error.contains("not connected"), "got: {error}");
}

#[tokio::test]
async fn meta_spawn_rejects_forged_parent_hub_before_lookup() {
    let mut params = spawn_payload("no-such-hub");
    params["parent"] = json!("hub-forged/parent");
    let error = spawn(params).await.expect_err("forged parent must fail");
    assert!(error.contains("authenticated hub 'hub-a'"), "got: {error}");
    assert!(!error.contains("not connected"), "got: {error}");
}

#[tokio::test]
async fn meta_spawn_rejects_local_parent_before_lookup() {
    let mut params = spawn_payload("no-such-hub");
    params["parent"] = json!("parent");
    let error = spawn(params).await.expect_err("local parent must fail");
    assert!(error.contains("remote QualifiedAddress"), "got: {error}");
    assert!(!error.contains("not connected"), "got: {error}");
}

#[tokio::test]
async fn valid_payload_reaches_target_lookup() {
    let error = spawn(spawn_payload("no-such-hub"))
        .await
        .expect_err("unregistered target must fail");
    assert!(error.contains("not connected"), "got: {error}");
}

#[tokio::test]
async fn valid_payload_is_stripped_and_forwarded() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (meta_transport, target_transport) = loopal_ipc::duplex_pair();
    let (meta_connection, _meta_rx) = Connection::new(meta_transport).into_listening();
    let (target_connection, mut target_rx) = Connection::new(target_transport).into_listening();
    meta_hub
        .lock()
        .await
        .registry
        .register("hub-b", meta_connection, vec![])
        .unwrap();
    let target = tokio::spawn(async move {
        let Incoming::Request { id, method, params } = target_rx.recv().await.unwrap() else {
            panic!("expected forwarded spawn request");
        };
        assert_eq!(method, methods::HUB_SPAWN_REMOTE_AGENT.name);
        assert!(params.get("target_hub").is_none());
        assert_eq!(params["parent"], "hub-a/parent");
        target_connection
            .respond(id, json!({"agent_id": "remote-id", "name": "child"}))
            .await
            .unwrap();
    });

    let result = dispatch_meta_request(
        &meta_hub,
        methods::META_SPAWN.name,
        spawn_payload("hub-b"),
        "hub-a".into(),
    )
    .await
    .unwrap();
    let outcome: loopal_ipc::cross_hub::RemoteSpawnOutcome =
        serde_json::from_value(result).unwrap();
    assert!(matches!(
        outcome,
        loopal_ipc::cross_hub::RemoteSpawnOutcome::Spawned { ref response }
            if response["agent_id"] == "remote-id"
    ));
    target.await.unwrap();
}
