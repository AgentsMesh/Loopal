use std::sync::Arc;

use loopal_ipc::Connection;
use loopal_protocol::AgentEvent;
use tokio::sync::{Mutex, mpsc};

use super::spawn::{spawn_io_loop, spawn_io_loop_exact};
use crate::Hub;

fn hub() -> Arc<Mutex<Hub>> {
    let (events, mut event_rx) = mpsc::channel::<AgentEvent>(16);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });
    Arc::new(Mutex::new(Hub::new(events)))
}

#[tokio::test]
async fn spawn_loop_without_registration_returns_without_side_effect() {
    let hub = hub();
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let (connection, incoming) = Connection::new(transport).into_listening();
    let dispatcher = Arc::new(crate::dispatch::build_hub_dispatcher(hub.clone()));
    spawn_io_loop(hub, dispatcher, "missing", connection, incoming);
    tokio::task::yield_now().await;
}

#[tokio::test]
async fn spawn_loop_and_exact_variant_finish_registered_generation() {
    for exact in [false, true] {
        let hub = hub();
        let (peer_transport, hub_transport) = loopal_ipc::duplex_pair();
        let (peer, _peer_rx) = Connection::new(peer_transport).into_listening();
        let (connection, incoming) = Connection::new(hub_transport).into_listening();
        let execution = hub
            .lock()
            .await
            .registry
            .register_connection_with_parent_execution(
                "worker",
                connection.clone(),
                None,
                None,
                None,
            )
            .unwrap();
        let dispatcher = Arc::new(crate::dispatch::build_hub_dispatcher(hub.clone()));
        if exact {
            spawn_io_loop_exact(
                hub.clone(),
                dispatcher,
                "worker",
                connection,
                incoming,
                execution,
            );
        } else {
            spawn_io_loop(hub.clone(), dispatcher, "worker", connection, incoming);
        }
        peer.send_notification(
            loopal_ipc::protocol::methods::AGENT_COMPLETED.name,
            serde_json::to_value(loopal_protocol::AgentCompletion::goal(Some("done".into())))
                .unwrap(),
        )
        .await
        .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while !matches!(
                hub.lock()
                    .await
                    .registry
                    .agent_info("worker")
                    .map(|info| &info.lifecycle),
                Some(crate::AgentLifecycle::Finished)
            ) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
}
