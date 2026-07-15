use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_agent_hub::{Hub, finish};
use loopal_ipc::connection::Connection;
use loopal_protocol::{AgentEvent, QualifiedAddress};
use tokio::sync::{Mutex, mpsc};

#[tokio::test]
async fn shadow_completion_records_once_without_a_bare_parent_envelope() {
    let (events, _) = mpsc::channel::<AgentEvent>(16);
    let hub = Arc::new(Mutex::new(Hub::new(events)));
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let (_client, mut parent_rx) = Connection::new(client_transport).into_listening();
    let (server, server_rx) = Connection::new(server_transport).into_listening();
    register_agent_connection(hub.clone(), "parent", server, server_rx, None, None, None)
        .await
        .unwrap();

    let mut completion = {
        let mut locked = hub.lock().await;
        locked
            .registry
            .register_shadow("remote-child", QualifiedAddress::local("parent"))
            .unwrap();
        locked.registry.watch_completion("remote-child")
    };
    finish::record_cross_hub_completion(&hub, "remote-child", "qualified result".into()).await;

    completion.changed().await.unwrap();
    assert_eq!(completion.borrow().as_deref(), Some("qualified result"));
    let locked = hub.lock().await;
    assert_eq!(
        locked.registry.completion_output("remote-child"),
        Some("qualified result")
    );
    let topology = locked.registry.topology_snapshot();
    let shadow = topology["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|agent| agent["name"] == "remote-child")
        .unwrap();
    assert_eq!(shadow["shadow"], true);
    drop(locked);
    assert!(
        tokio::time::timeout(Duration::from_millis(100), parent_rx.recv())
            .await
            .is_err()
    );
}
