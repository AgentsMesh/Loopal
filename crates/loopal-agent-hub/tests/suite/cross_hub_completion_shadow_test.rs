use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_agent_hub::{Hub, finish};
use loopal_ipc::connection::Connection;
use loopal_protocol::{AgentCompletion, AgentEvent, QualifiedAddress};
use tokio::sync::{Mutex, mpsc};

#[tokio::test]
async fn shadow_completion_records_once_without_a_bare_parent_envelope() {
    let (events, _event_rx) = mpsc::channel::<AgentEvent>(16);
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
    let inject_into_parent = finish::record_cross_hub_completion(
        &hub,
        "remote-child",
        AgentCompletion::new("error", Some("qualified partial result".into())),
    )
    .await;
    assert!(inject_into_parent);

    completion.changed().await.unwrap();
    let observed = completion.borrow().clone().unwrap();
    assert_eq!(observed.reason, "error");
    assert_eq!(observed.result.as_deref(), Some("qualified partial result"));
    let locked = hub.lock().await;
    assert_eq!(
        locked.registry.completion_output("remote-child"),
        Some("qualified partial result")
    );
    let topology = locked.registry.topology_snapshot();
    let shadow = topology["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|agent| agent["name"] == "remote-child")
        .unwrap();
    assert_eq!(shadow["shadow"], true);
    assert_eq!(shadow["lifecycle"], "failed");
    drop(locked);
    assert!(
        tokio::time::timeout(Duration::from_millis(100), parent_rx.recv())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn foreground_shadow_completion_resolves_wait_without_parent_push() {
    let (events, _event_rx) = mpsc::channel::<AgentEvent>(16);
    let hub = Arc::new(Mutex::new(Hub::new(events)));
    let mut completion = {
        let mut locked = hub.lock().await;
        locked
            .registry
            .register_shadow_with_parent_policy(
                "remote-foreground",
                QualifiedAddress::local("parent"),
                false,
            )
            .unwrap();
        locked.registry.watch_completion("remote-foreground")
    };

    let inject_into_parent = finish::record_cross_hub_completion(
        &hub,
        "remote-foreground",
        AgentCompletion::goal(Some("foreground result".into())),
    )
    .await;

    assert!(!inject_into_parent);
    completion.changed().await.unwrap();
    let observed = completion.borrow().clone().unwrap();
    assert_eq!(observed.reason, "goal");
    assert_eq!(observed.result.as_deref(), Some("foreground result"));

    let duplicate = finish::record_cross_hub_completion(
        &hub,
        "remote-foreground",
        AgentCompletion::goal(Some("duplicate must not reopen the agent".into())),
    )
    .await;
    assert!(!duplicate);
    assert_eq!(
        hub.lock()
            .await
            .registry
            .completion_output("remote-foreground"),
        Some("foreground result"),
    );
}
