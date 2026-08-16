use std::sync::Arc;

use loopal_ipc::connection::Connection;
use loopal_protocol::{AgentCompletion, AgentEvent};
use tokio::sync::mpsc;

use super::AgentRegistry;
use super::operations::PreparedInterrupt;

fn connection() -> Arc<Connection<loopal_ipc::connection::Listening>> {
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let (connection, _incoming) = Connection::new(transport).into_listening();
    connection
}

fn registry() -> AgentRegistry {
    let (event_tx, _event_rx) = mpsc::channel::<AgentEvent>(16);
    AgentRegistry::new(event_tx)
}

#[tokio::test]
async fn registration_returns_generation_from_the_insertion_transition() {
    let mut registry = registry();
    let lease = registry
        .register_connection_with_parent_execution("worker", connection(), None, None, None)
        .unwrap();

    assert_eq!(
        lease.address,
        loopal_protocol::QualifiedAddress::local("worker")
    );
    assert_eq!(registry.current_execution("worker"), Some(lease.clone()));
    assert!(registry.owns_active_lease(&lease));
}

#[tokio::test]
async fn stale_exact_parent_cannot_admit_child() {
    let mut registry = registry();
    let stale_parent = registry
        .register_connection_with_parent_execution("parent", connection(), None, None, None)
        .unwrap();
    assert!(registry.unregister_exact(&stale_parent));
    let current_parent = registry
        .register_connection_with_parent_execution("parent", connection(), None, None, None)
        .unwrap();

    let error = registry
        .register_connection_with_exact_parent_execution(
            "child",
            connection(),
            Some(stale_parent.address.clone()),
            Some(&stale_parent),
            None,
            None,
            true,
        )
        .unwrap_err();

    assert!(error.contains("stale"));
    assert!(registry.current_execution("child").is_none());
    assert!(registry.agent_info("parent").unwrap().children.is_empty());
    assert_eq!(registry.current_execution("parent"), Some(current_parent));
}

#[tokio::test]
async fn stale_lease_cannot_complete_or_unregister_reconnected_agent() {
    let mut registry = registry();
    let stale = registry
        .register_connection_with_parent_execution("worker", connection(), None, None, None)
        .unwrap();
    assert!(registry.unregister_exact(&stale));
    let current_connection = connection();
    let current = registry
        .register_connection_with_parent_execution(
            "worker",
            current_connection.clone(),
            None,
            None,
            None,
        )
        .unwrap();

    assert!(
        registry
            .emit_agent_completion_exact(&stale, AgentCompletion::goal(Some("stale".into())))
            .is_none()
    );
    assert!(!registry.unregister_exact(&stale));
    assert!(registry.completion("worker").is_none());
    assert!(Arc::ptr_eq(
        &registry.exact_connection(&current).unwrap(),
        &current_connection
    ));

    assert!(
        registry
            .emit_agent_completion_exact(&current, AgentCompletion::goal(Some("current".into())))
            .is_some()
    );
    assert_eq!(registry.completion_output("worker"), Some("current"));
    assert!(registry.unregister_exact(&current));
    assert!(registry.get_agent_connection("worker").is_none());
}

#[tokio::test]
async fn stale_lease_cannot_prepare_interrupt_or_shutdown_for_reconnect() {
    let mut registry = registry();
    let stale = registry
        .register_connection_with_parent_execution("worker", connection(), None, None, None)
        .unwrap();
    assert!(registry.unregister_exact(&stale));
    let current_connection = connection();
    let current = registry
        .register_connection_with_parent_execution(
            "worker",
            current_connection.clone(),
            None,
            None,
            None,
        )
        .unwrap();

    assert!(registry.interrupt_exact(&stale).is_none());
    assert!(registry.prepare_shutdown_exact(&stale).is_none());
    assert!(matches!(
        registry.interrupt_exact(&current),
        Some(PreparedInterrupt::Connected(ref connection))
            if Arc::ptr_eq(connection, &current_connection)
    ));
    assert!(Arc::ptr_eq(
        &registry.prepare_shutdown_exact(&current).unwrap(),
        &current_connection
    ));
}
