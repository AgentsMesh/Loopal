use std::sync::Arc;

use loopal_ipc::Connection;
use loopal_protocol::{AgentEvent, ControlCommand, InterruptSignal, QualifiedAddress};
use tokio::sync::{mpsc, watch};

use super::AgentRegistry;
use crate::LocalChannels;
use crate::topology::AgentLifecycle;
use crate::types::{AgentRuntimeFacts, SpawnAuthority};

fn registry() -> AgentRegistry {
    let (events, _rx) = mpsc::channel::<AgentEvent>(8);
    AgentRegistry::new(events)
}

fn connection() -> Arc<Connection<loopal_ipc::Listening>> {
    let (_peer, transport) = loopal_ipc::duplex_pair();
    Connection::new(transport).into_listening().0
}

fn local_channels() -> LocalChannels {
    let (control_tx, _control_rx) = mpsc::channel::<ControlCommand>(1);
    let (permission_tx, _permission_rx) = mpsc::channel(1);
    let (question_tx, _question_rx) = mpsc::channel(1);
    let (interrupt_tx, _) = watch::channel(0);
    LocalChannels {
        control_tx,
        permission_tx,
        question_tx,
        mailbox_tx: None,
        interrupt: InterruptSignal::new(),
        interrupt_tx: Arc::new(interrupt_tx),
    }
}

#[tokio::test]
async fn local_connected_and_shadow_queries_cover_all_states() {
    let mut registry = registry();
    registry.set_local("local", local_channels());
    let connected = connection();
    registry
        .register_connection("connected", connected.clone())
        .unwrap();
    registry
        .register_shadow("shadow", QualifiedAddress::local("connected"))
        .unwrap();

    assert_eq!(registry.agent_count(), 3);
    assert_eq!(registry.managed_agent_count(), 2);
    assert_eq!(registry.sub_agent_count(), 1);
    assert_eq!(registry.all_agent_connections().len(), 1);
    let states = registry.list_agents();
    assert!(states.contains(&("local".into(), "local")));
    assert!(states.contains(&("connected".into(), "connected")));
    assert!(states.contains(&("shadow".into(), "shadow")));
    assert!(registry.agent_view("local").is_some());
    assert!(registry.agent_view("missing").is_none());
}

#[tokio::test]
async fn runtime_facts_and_lifecycle_require_current_generation() {
    let mut registry = registry();
    let current = registry
        .register_connection_with_parent_execution("worker", connection(), None, None, None)
        .unwrap();
    let stale = crate::types::AgentExecutionRef::local("worker", current.connection_generation + 1);
    let facts = AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default());

    assert!(!registry.set_runtime_facts(&stale, facts.clone()));
    assert!(registry.runtime_facts(&stale).is_none());
    assert!(registry.set_runtime_facts(&current, facts));
    assert!(registry.runtime_facts(&current).is_some());
    registry.set_lifecycle("worker", AgentLifecycle::Running);
    assert_eq!(
        registry.agent_info("worker").unwrap().lifecycle,
        AgentLifecycle::Running
    );
    registry.set_lifecycle("missing", AgentLifecycle::Running);
}

#[tokio::test]
async fn interrupt_covers_missing_local_connected_and_shadow_states() {
    let mut registry = registry();
    registry.interrupt("missing").await;

    let channels = local_channels();
    let signal = channels.interrupt.clone();
    let mut generation = channels.interrupt_tx.subscribe();
    registry.set_local("local", channels);
    registry.interrupt("local").await;
    assert!(signal.is_signaled());
    generation.changed().await.unwrap();
    assert_eq!(*generation.borrow(), 1);

    let (peer, transport) = loopal_ipc::duplex_pair();
    let (peer, mut incoming) = Connection::new(peer).into_listening();
    let connected = Connection::new(transport).into_listening().0;
    registry
        .register_connection("connected", connected)
        .unwrap();
    registry.interrupt("connected").await;
    let loopal_ipc::connection::Incoming::Notification { method, .. } =
        incoming.recv().await.unwrap()
    else {
        panic!("expected interrupt notification");
    };
    assert_eq!(method, loopal_ipc::protocol::methods::AGENT_INTERRUPT.name);
    drop(peer);

    registry
        .register_shadow("shadow", QualifiedAddress::local("connected"))
        .unwrap();
    registry.interrupt("shadow").await;
}

#[tokio::test]
async fn descendants_ignore_stale_parent_generation_and_missing_roots() {
    let mut registry = registry();
    let first_parent = registry
        .register_connection_with_parent_execution("parent", connection(), None, None, None)
        .unwrap();
    registry
        .register_shadow("stale-child", QualifiedAddress::local("parent"))
        .unwrap();
    assert!(registry.unregister_exact(&first_parent));
    registry
        .register_connection_with_parent_execution("parent", connection(), None, None, None)
        .unwrap();
    registry
        .register_shadow("current-child", QualifiedAddress::local("parent"))
        .unwrap();

    assert_eq!(registry.descendants("parent"), ["current-child"]);
    assert!(registry.descendants("missing").is_empty());
    let snapshot = registry.topology_snapshot();
    assert!(
        snapshot["agents"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["name"] == "stale-child")
    );
}
