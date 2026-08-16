use std::sync::Arc;
use std::time::Duration;

use loopal_protocol::{AgentEvent, AgentEventPayload};
use tokio::sync::{Mutex, mpsc};

use super::{Registration, register};
use crate::Hub;
use crate::spawn_manager::SpawnRequestLease;
use crate::types::{AgentRuntimeFacts, SpawnAuthority};

fn registration() -> Registration {
    Registration {
        name: "cancelled-child".into(),
        request_lease: SpawnRequestLease::Internal,
        parent: None,
        expected_parent: None,
        model: None,
        session_id: None,
        notify_parent_on_completion: true,
        mark_running: true,
        facts: AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default()),
    }
}

#[tokio::test]
async fn cancelled_admission_cleans_exact_registration() {
    let (event_tx, _event_rx) = mpsc::channel(1);
    event_tx
        .send(AgentEvent::root(AgentEventPayload::Running))
        .await
        .unwrap();
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let (transport, _peer) = loopal_ipc::duplex_pair();
    let (connection, incoming) = loopal_ipc::Connection::new(transport).into_listening();
    let task = tokio::spawn(register(
        hub.clone(),
        connection.clone(),
        incoming,
        registration(),
    ));

    tokio::time::timeout(Duration::from_secs(1), async {
        while hub
            .lock()
            .await
            .registry
            .current_execution("cancelled-child")
            .is_none()
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());

    tokio::time::timeout(Duration::from_secs(1), async {
        while connection.is_connected() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(
        hub.lock()
            .await
            .registry
            .current_execution("cancelled-child")
            .is_none()
    );
}
