use std::sync::Arc;

use loopal_ipc::Connection;
use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use serde_json::{Value, json};
use tokio::sync::{Mutex, mpsc};

use super::{forward_action, handle};
use crate::request_principal::TrustedMetaHubPrincipal;
use crate::{Hub, HubUplink};

struct Fixture {
    hub: Arc<Mutex<Hub>>,
    active: TrustedMetaHubPrincipal,
    stale: TrustedMetaHubPrincipal,
    meta: Arc<Connection<loopal_ipc::Listening>>,
    meta_rx: mpsc::Receiver<Incoming>,
}

fn fixture() -> Fixture {
    let (hub_transport, meta_transport) = loopal_ipc::duplex_pair();
    let (hub_connection, _hub_rx) = Connection::new(hub_transport).into_listening();
    let (meta, meta_rx) = Connection::new(meta_transport).into_listening();
    let (stale_transport, _stale_peer) = loopal_ipc::duplex_pair();
    let (stale, _stale_rx) = Connection::new(stale_transport).into_listening();
    let active = TrustedMetaHubPrincipal::new(hub_connection.clone());
    let (event_tx, _event_rx) = mpsc::channel(8);
    let mut hub = Hub::new(event_tx);
    hub.uplink = Some(Arc::new(HubUplink::new(hub_connection, "hub-a".into())));
    Fixture {
        hub: Arc::new(Mutex::new(hub)),
        active,
        stale: TrustedMetaHubPrincipal::new(stale),
        meta,
        meta_rx,
    }
}

#[tokio::test]
async fn relay_requires_active_generation_and_allowlisted_operation() {
    let fixture = fixture();
    let error = handle(
        &fixture.hub,
        json!({"operation": "interrupt", "payload": {}}),
        &fixture.stale,
    )
    .await
    .unwrap_err();
    assert!(error.contains("stale uplink generation"));

    let cases = [
        (json!({"operation": "question_request"}), "origin_hub"),
        (
            json!({"operation": "question_response", "payload": {}}),
            "agent_name",
        ),
        (json!({"operation": "question_cancel"}), "origin_hub"),
        (json!({"operation": "control", "payload": {}}), "target"),
        (json!({"operation": "interrupt", "payload": {}}), "target"),
    ];
    for (params, expected) in cases {
        let error = handle(&fixture.hub, params, &fixture.active)
            .await
            .unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
    let error = handle(
        &fixture.hub,
        json!({"operation": "future_admin"}),
        &fixture.active,
    )
    .await
    .unwrap_err();
    assert!(error.contains("unsupported remote relay operation: future_admin"));
}

#[tokio::test]
async fn forward_action_rewrites_next_hop_and_local_target() {
    let fixture = fixture();
    let hub = fixture.hub.clone();
    let meta = fixture.meta.clone();
    let mut meta_rx = fixture.meta_rx;
    let responder = tokio::spawn(async move {
        let Incoming::Request { id, method, params } = meta_rx.recv().await.unwrap() else {
            panic!("expected relay request");
        };
        assert_eq!(method, methods::META_REMOTE_RELAY.name);
        assert_eq!(params["target_hub"], "next-hub");
        assert_eq!(params["operation"], "interrupt");
        assert_eq!(params["payload"]["target"], "worker");
        assert_eq!(params["payload"]["extra"], 1);
        meta.respond(id, json!({"ok": true})).await.unwrap();
    });

    let result = forward_action(
        &hub,
        "next-hub/worker",
        "interrupt",
        json!({"target": "next-hub/worker", "extra": 1}),
    )
    .await
    .unwrap();
    assert_eq!(result["ok"], true);
    responder.await.unwrap();

    hub.lock().await.uplink = None;
    let error = forward_action(&hub, "next-hub/worker", "interrupt", Value::Null)
        .await
        .unwrap_err();
    assert!(error.contains("remote target required"));
}
