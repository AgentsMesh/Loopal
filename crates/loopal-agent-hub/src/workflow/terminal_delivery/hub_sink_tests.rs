use std::sync::Arc;

use loopal_ipc::{Connection, connection::Incoming};
use loopal_protocol::{
    ROOT_AGENT_NAME, WorkflowFailureClass, WorkflowRunId, WorkflowRunState,
    WorkflowTerminalDeliveryId, WorkflowTerminalDisposition, WorkflowTerminalNotification,
    WorkflowTerminalOutcome,
};
use tokio::sync::{Mutex, mpsc};

use super::{HubWorkflowTerminalSink, WorkflowTerminalSink};
use crate::Hub;
use crate::types::{AgentRuntimeFacts, SpawnAuthority};
use crate::workflow::WorkflowOwner;

fn owner() -> WorkflowOwner {
    WorkflowOwner::new(
        "session",
        loopal_protocol::QualifiedAddress::local(ROOT_AGENT_NAME),
    )
}

fn notification() -> WorkflowTerminalNotification {
    WorkflowTerminalNotification {
        delivery_id: WorkflowTerminalDeliveryId::new("session", WorkflowRunId::new("run"), 1),
        state: WorkflowRunState::Failed,
        run_goal: "goal".into(),
        outcome: WorkflowTerminalOutcome::Failed {
            class: WorkflowFailureClass::Permanent,
            reason: "failure".into(),
        },
        content: "failure".into(),
    }
}

async fn authenticated_root() -> (
    Arc<Mutex<Hub>>,
    Arc<Connection<loopal_ipc::Listening>>,
    Arc<Connection<loopal_ipc::Listening>>,
    tokio::sync::mpsc::Receiver<Incoming>,
) {
    let (events, _event_rx) = mpsc::channel(8);
    let mut hub = Hub::new(events);
    let (peer, transport) = loopal_ipc::duplex_pair();
    let (peer_connection, incoming) = Connection::new(peer).into_listening();
    let connection = Connection::new(transport).into_listening().0;
    let execution = hub
        .registry
        .register_connection_with_parent_execution(
            ROOT_AGENT_NAME,
            connection.clone(),
            None,
            None,
            None,
        )
        .unwrap();
    let mut facts = AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default());
    facts.session_id = Some("session".into());
    assert!(hub.registry.set_runtime_facts(&execution, facts));
    (
        Arc::new(Mutex::new(hub)),
        connection,
        peer_connection,
        incoming,
    )
}

#[tokio::test]
async fn exact_connection_requires_the_current_authenticated_root() {
    let (events, _event_rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(events)));
    let sink = HubWorkflowTerminalSink::new(hub.clone());
    let missing = sink.exact_connection(&owner()).await;
    assert!(matches!(
        missing,
        Err(ref error) if error == "workflow root Agent is not connected"
    ));

    let (_peer, transport) = loopal_ipc::duplex_pair();
    let connection = Connection::new(transport).into_listening().0;
    let execution = hub
        .lock()
        .await
        .registry
        .register_connection_with_parent_execution(ROOT_AGENT_NAME, connection, None, None, None)
        .unwrap();
    let unauthenticated = sink.exact_connection(&owner()).await;
    assert!(matches!(
        unauthenticated,
        Err(ref error) if error == "workflow root Agent has no authenticated runtime facts"
    ));

    let mut facts = AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default());
    facts.session_id = Some("other-session".into());
    assert!(
        hub.lock()
            .await
            .registry
            .set_runtime_facts(&execution, facts)
    );
    let changed = sink.exact_connection(&owner()).await;
    assert!(matches!(
        changed,
        Err(ref error) if error == "workflow root Agent authority changed"
    ));
}

#[tokio::test]
async fn exact_connection_and_still_exact_bind_the_same_root_lease() {
    let (hub, connection, _peer, _incoming) = authenticated_root().await;
    let sink = HubWorkflowTerminalSink::new(hub.clone());
    let (execution, selected) = sink.exact_connection(&owner()).await.unwrap();
    assert!(Arc::ptr_eq(&connection, &selected));
    assert!(sink.still_exact(&owner(), &execution).await);

    let mut facts = hub
        .lock()
        .await
        .registry
        .runtime_facts(&execution)
        .unwrap()
        .clone();
    facts.session_id = Some("other-session".into());
    assert!(
        hub.lock()
            .await
            .registry
            .set_runtime_facts(&execution, facts)
    );
    assert!(!sink.still_exact(&owner(), &execution).await);
}

#[tokio::test]
async fn deliver_round_trips_the_terminal_disposition_to_the_exact_root() {
    let (hub, _connection, peer, mut incoming) = authenticated_root().await;
    let sink = HubWorkflowTerminalSink::new(hub);
    let responder = tokio::spawn(async move {
        let Some(Incoming::Request { id, method, params }) = incoming.recv().await else {
            panic!("expected terminal request");
        };
        assert_eq!(
            method,
            loopal_ipc::protocol::methods::AGENT_WORKFLOW_TERMINAL.name
        );
        assert_eq!(
            serde_json::from_value::<WorkflowTerminalNotification>(params).unwrap(),
            notification()
        );
        peer.respond(
            id,
            serde_json::to_value(WorkflowTerminalDisposition::Applied).unwrap(),
        )
        .await
        .unwrap();
    });
    assert_eq!(
        sink.deliver(&owner(), notification()).await,
        Ok(WorkflowTerminalDisposition::Applied)
    );
    responder.await.unwrap();
}
