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
    let left = Connection::new(left).into_listening();
    let right = Connection::new(right).into_listening();
    (left, right)
}

fn result(text: &str, typed: bool) -> serde_json::Value {
    let envelope = Envelope::new(
        MessageSource::AgentResult {
            child: QualifiedAddress::local("remote-child"),
        },
        QualifiedAddress::local("parent"),
        text,
    );
    serde_json::to_value(if typed {
        envelope.with_agent_completion(loopal_protocol::AgentCompletion::goal(Some(text.into())))
    } else {
        envelope
    })
    .unwrap()
}

#[tokio::test]
async fn result_is_scoped_to_active_uplink_and_parent_generation() {
    let (events, mut event_rx) = mpsc::channel::<AgentEvent>(16);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });
    let hub = Arc::new(Mutex::new(Hub::new(events)));
    let ((parent_peer, mut parent_rx), (parent_hub, _)) = pair();
    hub.lock()
        .await
        .registry
        .register_connection("parent", parent_hub)
        .unwrap();
    hub.lock()
        .await
        .registry
        .register_shadow("remote-child", QualifiedAddress::local("parent"))
        .unwrap();
    let ((reverse, reverse_rx), (meta, _)) = pair();
    let ((stale, _), _) = pair();
    hub.lock().await.uplink = Some(Arc::new(HubUplink::new(stale, "hub-a".into())));
    tokio::spawn(handle_reverse_requests(
        hub.clone(),
        reverse.clone(),
        reverse_rx,
        "hub-a".into(),
    ));

    assert_eq!(
        meta.send_request(methods::AGENT_MESSAGE.name, result("stale", true))
            .await
            .unwrap()["ok"],
        true
    );
    assert!(
        hub.lock()
            .await
            .registry
            .completion("remote-child")
            .is_none()
    );
    hub.lock().await.uplink = Some(Arc::new(HubUplink::new(reverse, "hub-a".into())));
    let responder = tokio::spawn(async move {
        let Incoming::Request { id, .. } = parent_rx.recv().await.unwrap() else {
            panic!("expected parent completion");
        };
        parent_peer
            .respond(id, serde_json::json!({"ok": true}))
            .await
            .unwrap();
    });
    assert_eq!(
        meta.send_request(methods::AGENT_MESSAGE.name, result("done", false))
            .await
            .unwrap()["ok"],
        true
    );
    responder.await.unwrap();
    assert_eq!(
        hub.lock().await.registry.completion_output("remote-child"),
        Some("done")
    );
}
