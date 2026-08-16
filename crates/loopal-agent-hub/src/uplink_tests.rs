use std::sync::Arc;

use loopal_ipc::Connection;
use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, Envelope, MessageSource, QualifiedAddress};
use tokio::sync::{Mutex, mpsc};

use super::{HubUplink, handle_reverse_requests};
use crate::Hub;

type IpcEndpoint = (
    Arc<Connection<loopal_ipc::Listening>>,
    mpsc::Receiver<Incoming>,
);

fn pair() -> (IpcEndpoint, IpcEndpoint) {
    let (left, right) = loopal_ipc::duplex_pair();
    let (left, left_rx) = Connection::new(left).into_listening();
    let (right, right_rx) = Connection::new(right).into_listening();
    ((left, left_rx), (right, right_rx))
}

fn hub() -> Arc<Mutex<Hub>> {
    let (events, mut rx) = mpsc::channel::<AgentEvent>(16);
    tokio::spawn(async move { while rx.recv().await.is_some() {} });
    Arc::new(Mutex::new(Hub::new(events)))
}

fn envelope(target: &str, text: &str) -> serde_json::Value {
    serde_json::to_value(Envelope::new(
        MessageSource::Human,
        QualifiedAddress::local(target),
        text,
    ))
    .unwrap()
}

#[tokio::test]
async fn heartbeat_sends_hub_name_and_agent_count() {
    let ((hub_conn, _hub_rx), (meta, mut meta_rx)) = pair();
    let uplink = HubUplink::with_address(hub_conn.clone(), "hub-a".into(), "meta:1".into());
    assert_eq!(uplink.hub_name(), "hub-a");
    assert_eq!(uplink.meta_address(), Some("meta:1"));
    assert!(Arc::ptr_eq(uplink.connection(), &hub_conn));

    tokio::spawn(async move {
        let Incoming::Request { id, method, params } = meta_rx.recv().await.unwrap() else {
            panic!("expected heartbeat request");
        };
        assert_eq!(method, methods::META_HEARTBEAT.name);
        assert_eq!(params["hub_name"], "hub-a");
        assert_eq!(params["agent_count"], 7);
        meta.respond(id, serde_json::json!({"ok": true}))
            .await
            .unwrap();
    });
    uplink.heartbeat(7).await.unwrap();
}

#[tokio::test]
async fn reverse_request_and_notification_deliver_to_local_agent() {
    let hub = hub();
    let ((agent_peer, mut agent_rx), (agent_hub, _agent_hub_rx)) = pair();
    hub.lock()
        .await
        .registry
        .register_connection("target", agent_hub)
        .unwrap();
    let ((reverse, reverse_rx), (meta, _meta_rx)) = pair();
    tokio::spawn(handle_reverse_requests(
        hub,
        reverse,
        reverse_rx,
        "hub-a".into(),
    ));
    let responder = tokio::spawn(async move {
        for _ in 0..2 {
            let Incoming::Request { id, method, params } = agent_rx.recv().await.unwrap() else {
                panic!("expected routed request");
            };
            assert_eq!(method, methods::AGENT_MESSAGE.name);
            assert_eq!(params["target"]["agent"], "target");
            agent_peer
                .respond(id, serde_json::json!({"ok": true}))
                .await
                .unwrap();
        }
    });

    let result = meta
        .send_request(methods::AGENT_MESSAGE.name, envelope("target", "request"))
        .await
        .unwrap();
    assert_eq!(result["ok"], true);
    meta.send_notification(methods::AGENT_MESSAGE.name, envelope("target", "notice"))
        .await
        .unwrap();
    responder.await.unwrap();
}

#[tokio::test]
async fn reverse_requests_fail_closed_for_invalid_message_and_hub_method() {
    let hub = hub();
    let ((reverse, reverse_rx), (meta, _meta_rx)) = pair();
    hub.lock().await.uplink = Some(Arc::new(HubUplink::new(reverse.clone(), "hub-a".into())));
    tokio::spawn(handle_reverse_requests(
        hub,
        reverse,
        reverse_rx,
        "hub-a".into(),
    ));

    let invalid = meta
        .send_request(
            methods::AGENT_MESSAGE.name,
            serde_json::json!({"bad": true}),
        )
        .await
        .unwrap();
    assert_eq!(invalid["ok"], false);
    let error = meta
        .send_request(methods::HUB_STATUS.name, serde_json::Value::Null)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("not authorized"));
    meta.send_notification(methods::REQUEST_CANCEL.name, serde_json::json!({"id": 1}))
        .await
        .unwrap();
    let after_cancel = meta
        .send_request(
            methods::AGENT_MESSAGE.name,
            serde_json::json!({"bad": true}),
        )
        .await
        .unwrap();
    assert_eq!(after_cancel["ok"], false);
}
