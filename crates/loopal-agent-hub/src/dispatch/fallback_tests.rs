use std::sync::Arc;

use loopal_ipc::Connection;
use loopal_ipc::connection::Incoming;
use loopal_protocol::AgentEvent;
use tokio::sync::{Mutex, mpsc};

use super::{dispatch_hub_request, handle_fallback};
use crate::{Hub, HubUplink};

fn hub() -> Arc<Mutex<Hub>> {
    let (events, _rx) = mpsc::channel::<AgentEvent>(8);
    Arc::new(Mutex::new(Hub::new(events)))
}

#[tokio::test]
async fn fallback_rejects_unknown_and_disconnected_meta_methods() {
    let hub = hub();
    let unknown = dispatch_hub_request(
        &hub,
        "hub/unknown",
        serde_json::Value::Null,
        "internal".into(),
    )
    .await
    .unwrap_err();
    assert!(unknown.contains("unknown hub method"));
    let disconnected = handle_fallback(hub, "meta/query".into(), serde_json::Value::Null)
        .await
        .unwrap_err();
    assert!(disconnected.to_string().contains("not connected"));
}

#[tokio::test]
async fn fallback_forwards_success_and_embedded_meta_error() {
    for embedded_error in [false, true] {
        let hub = hub();
        let (hub_transport, meta_transport) = loopal_ipc::duplex_pair();
        let (hub_conn, _hub_rx) = Connection::new(hub_transport).into_listening();
        let (meta, mut meta_rx) = Connection::new(meta_transport).into_listening();
        hub.lock().await.uplink = Some(Arc::new(HubUplink::new(hub_conn, "hub".into())));
        tokio::spawn(async move {
            let Incoming::Request { id, method, .. } = meta_rx.recv().await.unwrap() else {
                panic!("expected meta request");
            };
            assert_eq!(method, "meta/query");
            let response = if embedded_error {
                serde_json::json!({"message": "failed"})
            } else {
                serde_json::json!({"ok": true})
            };
            meta.respond(id, response).await.unwrap();
        });
        let result = handle_fallback(hub, "meta/query".into(), serde_json::Value::Null).await;
        if embedded_error {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("meta/query error: failed")
            );
        } else {
            assert_eq!(result.unwrap()["ok"], true);
        }
    }
}
