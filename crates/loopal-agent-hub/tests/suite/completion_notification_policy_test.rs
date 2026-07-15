use std::sync::Arc;

use loopal_agent_hub::AgentRegistry;
use loopal_ipc::Connection;
use loopal_protocol::{AgentEvent, Envelope, QualifiedAddress};
use tokio::sync::mpsc;

fn connection() -> Arc<Connection<loopal_ipc::connection::Listening>> {
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let (connection, _incoming) = Connection::new(transport).into_listening();
    connection
}

#[tokio::test]
async fn internal_child_keeps_parent_topology_without_injecting_completion() {
    let (event_tx, _event_rx) = mpsc::channel::<AgentEvent>(8);
    let (parent_tx, mut parent_rx) = mpsc::channel::<Envelope>(2);
    let mut registry = AgentRegistry::new(event_tx);
    registry
        .register_connection_with_parent("main", connection(), None, None, Some(parent_tx))
        .unwrap();
    registry
        .register_connection_with_parent_policy(
            "memory-worker",
            connection(),
            Some(QualifiedAddress::local("main")),
            Some("model"),
            None,
            false,
        )
        .unwrap();
    let completion = registry.watch_completion("memory-worker");

    let delivery =
        registry.emit_agent_finished("memory-worker", Some("memory maintenance complete".into()));

    assert!(delivery.is_none());
    assert!(parent_rx.try_recv().is_err());
    assert_eq!(
        completion.borrow().as_deref(),
        Some("memory maintenance complete"),
    );
    assert_eq!(
        registry.agent_info("memory-worker").unwrap().parent,
        Some(QualifiedAddress::local("main")),
    );
    assert_eq!(
        registry.agent_info("main").unwrap().children,
        vec!["memory-worker"],
    );
}
