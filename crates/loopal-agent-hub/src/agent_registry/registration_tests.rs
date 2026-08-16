use std::sync::Arc;

use loopal_ipc::Connection;
use loopal_protocol::{AgentEvent, ControlCommand, InterruptSignal};
use tokio::sync::{mpsc, watch};

use super::AgentRegistry;
use crate::LocalChannels;

type ListeningConnection = Arc<Connection<loopal_ipc::Listening>>;

fn registry() -> AgentRegistry {
    let (events, _rx) = mpsc::channel::<AgentEvent>(8);
    AgentRegistry::new(events)
}

fn pair() -> (ListeningConnection, ListeningConnection) {
    let (left, right) = loopal_ipc::duplex_pair();
    (
        Connection::new(left).into_listening().0,
        Connection::new(right).into_listening().0,
    )
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
async fn live_reservation_rejects_competing_registration_and_reservation() {
    let mut registry = registry();
    let (reserved, _reserved_peer) = pair();
    let (competitor, _competitor_peer) = pair();
    registry
        .reserve_connection("worker", reserved.clone())
        .unwrap();

    assert!(
        registry
            .register_connection_with_parent_policy_execution(
                "worker",
                competitor.clone(),
                None,
                None,
                None,
                true,
            )
            .unwrap_err()
            .contains("pending")
    );
    assert!(
        registry
            .register_connection_with_exact_parent_execution(
                "worker",
                competitor.clone(),
                None,
                None,
                None,
                None,
                true,
            )
            .unwrap_err()
            .contains("pending")
    );
    assert!(
        registry
            .reserve_connection("worker", competitor)
            .unwrap_err()
            .contains("already pending")
    );

    registry.set_local("registered", local_channels());
    assert!(
        registry
            .reserve_connection("registered", reserved)
            .unwrap_err()
            .contains("already registered")
    );
}

#[tokio::test]
async fn activation_and_cancellation_require_the_exact_reserved_connection() {
    let mut registry = registry();
    let (reserved, _reserved_peer) = pair();
    let (other, _other_peer) = pair();
    let (completion_tx, _completion_rx) = mpsc::channel(1);

    assert!(
        registry
            .activate_reserved_connection_with_execution(
                "missing",
                reserved.clone(),
                completion_tx.clone(),
            )
            .unwrap_err()
            .contains("was lost")
    );
    registry
        .reserve_connection("worker", reserved.clone())
        .unwrap();
    assert!(
        registry
            .activate_reserved_connection_with_execution(
                "worker",
                other.clone(),
                completion_tx.clone(),
            )
            .unwrap_err()
            .contains("changed owner")
    );
    registry.set_local("worker", local_channels());
    assert!(
        registry
            .activate_reserved_connection_with_execution("worker", reserved.clone(), completion_tx,)
            .unwrap_err()
            .contains("already registered")
    );

    let (cancelled, _cancelled_peer) = pair();
    registry
        .reserve_connection("cancelled", cancelled.clone())
        .unwrap();
    assert!(!registry.cancel_connection_reservation("cancelled", &other));
    assert!(registry.cancel_connection_reservation("cancelled", &cancelled));
    assert!(!registry.cancel_connection_reservation("cancelled", &cancelled));
}
