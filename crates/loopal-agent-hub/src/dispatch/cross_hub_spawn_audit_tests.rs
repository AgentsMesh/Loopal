use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::Connection;
use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_vault_api::ProtectedOp;
use tokio::sync::{Mutex, mpsc};

use super::forward_cross_hub_spawn;
use super::tests::{hub_with_uplink_and_audit, signed_spawn};
use crate::spawn_manager::spawn_audit_test_support::Sink;
use crate::{Hub, HubUplink};

async fn assert_shadow(hub: &Arc<Mutex<Hub>>, name: &str, exists: bool) {
    assert_eq!(hub.lock().await.registry.agent_info(name).is_some(), exists);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn audit_precedes_shadow_and_remote_rpc_with_authenticated_metadata() {
    let (event_tx, _event_rx) = mpsc::channel(8);
    let (sink, gate) = Sink::gated();
    let sink = Arc::new(sink);
    let (hub, meta, mut meta_rx, requester) =
        hub_with_uplink_and_audit(event_tx, Some(sink.clone())).await;
    let expected_cwd = hub.lock().await.default_cwd.clone();
    let request_generation = requester.connection_generation;
    let spawn = tokio::spawn({
        let hub = hub.clone();
        async move {
            let mut params = signed_spawn("audited-worker");
            params["prompt"] = serde_json::json!("prompt-canary");
            forward_cross_hub_spawn(&hub, params, &requester).await
        }
    });

    gate.wait_started().await;
    let locked = tokio::time::timeout(Duration::from_millis(200), hub.lock())
        .await
        .expect("audit append must not hold the Hub mutex");
    assert!(locked.registry.agent_info("audited-worker").is_none());
    drop(locked);
    assert!(meta_rx.try_recv().is_err());

    let records = sink.records();
    let record = records.first().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(record.op, ProtectedOp::SpawnAuthority);
    assert_eq!(record.subject, "audited-worker");
    assert_eq!(record.session_id.as_deref(), Some("session-cross-hub"));
    assert_eq!(record.cwd.as_deref(), Some(expected_cwd.as_path()));
    assert_eq!(record.agent_name.as_deref(), Some("main"));
    assert_eq!(record.depth, Some(1));
    assert_eq!(record.generation, Some(request_generation));
    assert_eq!(record.workflow_run_id.as_deref(), Some("wrun_cross_hub"));
    assert_eq!(record.workflow_node_id.as_deref(), Some("wnode_cross_hub"));
    assert_eq!(
        record.workflow_attempt_id.as_deref(),
        Some("watt_cross_hub")
    );
    assert_eq!(record.spawn_target.as_deref(), Some("hub:destination"));
    assert_eq!(record.model.as_deref(), Some("test-model"));
    assert_eq!(record.permission_mode.as_deref(), Some("ask_any_write"));
    assert_eq!(record.decision_mode.as_deref(), Some("manual"));
    assert_eq!(record.sandbox_policy.as_deref(), Some("read_only"));

    gate.release();
    let Incoming::Request { id, method, .. } = meta_rx.recv().await.unwrap() else {
        panic!("expected meta/spawn request");
    };
    assert_eq!(method, methods::META_SPAWN.name);
    meta.respond(id, serde_json::json!({"agent_id": "remote-id"}))
        .await
        .unwrap();
    assert_eq!(spawn.await.unwrap().unwrap()["agent_id"], "remote-id");
    assert_shadow(&hub, "audited-worker", true).await;
}

#[tokio::test]
async fn missing_or_failing_audit_leaves_no_shadow_or_remote_rpc() {
    let (event_tx, _event_rx) = mpsc::channel(8);
    let (hub, _meta, mut meta_rx, requester) = hub_with_uplink_and_audit(event_tx, None).await;
    let error = forward_cross_hub_spawn(&hub, signed_spawn("missing-audit"), &requester)
        .await
        .unwrap_err();
    assert_eq!(error, "protected audit unavailable");
    assert_shadow(&hub, "missing-audit", false).await;
    assert!(meta_rx.try_recv().is_err());

    let (event_tx, _event_rx) = mpsc::channel(8);
    let sink = Arc::new(Sink::new(true));
    let (hub, _meta, mut meta_rx, requester) =
        hub_with_uplink_and_audit(event_tx, Some(sink.clone())).await;
    let error = forward_cross_hub_spawn(&hub, signed_spawn("failed-audit"), &requester)
        .await
        .unwrap_err();
    assert!(error.contains("spawn authority audit failed"));
    assert_eq!(sink.records().len(), 1);
    assert_shadow(&hub, "failed-audit", false).await;
    assert!(meta_rx.try_recv().is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn name_conflicts_are_checked_before_and_after_audit() {
    let (event_tx, _event_rx) = mpsc::channel(8);
    let sink = Arc::new(Sink::new(false));
    let (hub, _meta, mut meta_rx, requester) =
        hub_with_uplink_and_audit(event_tx, Some(sink.clone())).await;
    hub.lock()
        .await
        .registry
        .register_shadow("claimed", loopal_protocol::QualifiedAddress::local("main"))
        .unwrap();
    let error = forward_cross_hub_spawn(&hub, signed_spawn("claimed"), &requester)
        .await
        .unwrap_err();
    assert_eq!(error, "agent 'claimed' already registered");
    assert!(sink.records().is_empty());
    assert!(meta_rx.try_recv().is_err());

    let (sink, gate) = Sink::gated();
    hub.lock().await.set_protected_audit(Arc::new(sink));
    let spawn = tokio::spawn({
        let hub = hub.clone();
        async move { forward_cross_hub_spawn(&hub, signed_spawn("raced"), &requester).await }
    });
    gate.wait_started().await;
    hub.lock()
        .await
        .registry
        .register_shadow("raced", loopal_protocol::QualifiedAddress::local("main"))
        .unwrap();
    gate.release();
    assert_eq!(
        spawn.await.unwrap().unwrap_err(),
        "agent 'raced' already registered"
    );
    assert!(meta_rx.try_recv().is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn requester_replacement_during_audit_leaves_no_shadow_or_remote_rpc() {
    let (event_tx, _event_rx) = mpsc::channel(8);
    let (sink, gate) = Sink::gated();
    let (hub, _meta, mut meta_rx, requester) =
        hub_with_uplink_and_audit(event_tx, Some(Arc::new(sink))).await;
    let stale = requester.clone();
    let spawn = tokio::spawn({
        let hub = hub.clone();
        async move { forward_cross_hub_spawn(&hub, signed_spawn("stale-audit"), &requester).await }
    });

    gate.wait_started().await;
    let mut locked = hub.lock().await;
    assert!(locked.registry.unregister_exact(&stale));
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let (replacement, _incoming) = Connection::new(transport).into_listening();
    locked
        .registry
        .register_connection("main", replacement)
        .unwrap();
    drop(locked);
    gate.release();

    let error = spawn.await.unwrap().unwrap_err();
    assert_eq!(error, "spawn requester connection lease is stale");
    assert_shadow(&hub, "stale-audit", false).await;
    assert!(meta_rx.try_recv().is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn uplink_replacement_during_audit_leaves_no_shadow_or_remote_rpc() {
    let (event_tx, _event_rx) = mpsc::channel(8);
    let (sink, gate) = Sink::gated();
    let (hub, _meta, mut meta_rx, requester) =
        hub_with_uplink_and_audit(event_tx, Some(Arc::new(sink))).await;
    let spawn = tokio::spawn({
        let hub = hub.clone();
        async move { forward_cross_hub_spawn(&hub, signed_spawn("uplink-audit"), &requester).await }
    });

    gate.wait_started().await;
    let (hub_transport, meta_transport) = loopal_ipc::duplex_pair();
    let (replacement, _replacement_rx) = Connection::new(hub_transport).into_listening();
    let (_replacement_meta, _meta_rx) = Connection::new(meta_transport).into_listening();
    hub.lock().await.uplink = Some(Arc::new(HubUplink::new(replacement, "new-origin".into())));
    gate.release();

    let error = spawn.await.unwrap().unwrap_err();
    assert_eq!(
        error,
        "MetaHub uplink changed during remote spawn admission"
    );
    assert_shadow(&hub, "uplink-audit", false).await;
    assert!(meta_rx.try_recv().is_err());
}
