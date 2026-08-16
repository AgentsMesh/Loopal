use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::{HubUplink, UiSession, start_event_loop};
use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use serde_json::json;

use crate::test_helpers::*;

#[tokio::test]
async fn qualified_remote_interrupt_request_is_acknowledged_by_child_agent() {
    let cluster = cluster().await;
    let ui = UiSession::connect(
        cluster.hub_a.clone(),
        "desktop-ui",
        loopal_protocol::UiCapabilities::NONE,
    )
    .await;
    let (agent, mut incoming) = register_remote_agent(
        &cluster.hub_b,
        "remote-worker",
        loopal_protocol::QualifiedAddress::remote(["hub-a"], "main"),
    )
    .await;
    let responder = tokio::spawn(async move {
        let message = incoming
            .recv()
            .await
            .expect("remote agent connection closed");
        let Incoming::Request { id, method, .. } = message else {
            panic!("expected interrupt request")
        };
        assert_eq!(method, methods::AGENT_INTERRUPT.name);
        agent.respond(id, json!({"ok": true})).await.unwrap();
    });
    ui.client
        .connection()
        .send_request(
            methods::HUB_INTERRUPT.name,
            json!({"target": "hub-b/remote-worker"}),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), responder)
        .await
        .expect("remote interrupt was not acknowledged")
        .unwrap();
}

struct Cluster {
    hub_a: Arc<tokio::sync::Mutex<loopal_agent_hub::Hub>>,
    hub_b: Arc<tokio::sync::Mutex<loopal_agent_hub::Hub>>,
}

async fn cluster() -> Cluster {
    let meta = Arc::new(tokio::sync::Mutex::new(loopal_meta_hub::MetaHub::new()));
    let (hub_a, events_a) = make_hub();
    let (hub_b, events_b) = make_hub();
    let _loop_a = start_event_loop(hub_a.clone(), events_a);
    let _loop_b = start_event_loop(hub_b.clone(), events_b);
    let conn_a = wire_hub_to_meta("hub-a", &hub_a, &meta).await;
    let conn_b = wire_hub_to_meta("hub-b", &hub_b, &meta).await;
    hub_a.lock().await.uplink = Some(Arc::new(HubUplink::new(conn_a, "hub-a".into())));
    hub_b.lock().await.uplink = Some(Arc::new(HubUplink::new(conn_b, "hub-b".into())));
    Cluster { hub_a, hub_b }
}
