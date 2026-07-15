use std::sync::{Arc, Weak};
use std::time::Duration;

use loopal_agent_hub::{AgentLifecycle, AgentRegistry, Hub, UiSession, start_event_loop};
use loopal_ipc::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload, AgentStatus, QualifiedAddress};
use loopal_view_state::{ViewSnapshot, ViewSnapshotRequest};
use tokio::sync::{Mutex, broadcast, mpsc};

fn make_hub() -> (
    Arc<Mutex<Hub>>,
    mpsc::Sender<AgentEvent>,
    mpsc::Receiver<AgentEvent>,
) {
    let (tx, rx) = mpsc::channel(32);
    (Arc::new(Mutex::new(Hub::new(tx.clone()))), tx, rx)
}

async fn register(
    hub: &Arc<Mutex<Hub>>,
    name: &str,
    parent: Option<&str>,
) -> Weak<Connection<loopal_ipc::connection::Listening>> {
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let (connection, _incoming) = Connection::new(transport).into_listening();
    let weak = Arc::downgrade(&connection);
    let mut hub = hub.lock().await;
    hub.registry
        .register_connection_with_parent(
            name,
            connection,
            parent.map(QualifiedAddress::local),
            None,
            None,
        )
        .unwrap();
    hub.registry.set_lifecycle(name, AgentLifecycle::Running);
    weak
}

async fn publish(
    tx: &mpsc::Sender<AgentEvent>,
    events: &mut broadcast::Receiver<AgentEvent>,
    name: &str,
    payload: AgentEventPayload,
) {
    tx.send(AgentEvent::named(name, payload)).await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .expect("event timeout")
        .expect("event broadcast");
}

async fn cold_state(hub: Arc<Mutex<Hub>>, agent: &str) -> (ViewSnapshot, serde_json::Value) {
    let ui = UiSession::connect(hub, "cold-ui").await;
    let response = ui
        .client
        .connection()
        .send_request(
            methods::VIEW_SNAPSHOT.name,
            serde_json::to_value(ViewSnapshotRequest {
                agent: agent.into(),
            })
            .unwrap(),
        )
        .await
        .expect("retained snapshot");
    let topology = ui
        .client
        .connection()
        .send_request(methods::HUB_TOPOLOGY.name, serde_json::json!({}))
        .await
        .expect("retained topology");
    (serde_json::from_value(response).unwrap(), topology)
}

fn has_message(snapshot: &ViewSnapshot, role: &str, content: &str) -> bool {
    snapshot
        .state
        .agent
        .conversation
        .messages
        .iter()
        .any(|message| message.role == role && message.content == content)
}

#[tokio::test]
async fn finished_child_is_queryable_after_unregister() {
    let (hub, tx, rx) = make_hub();
    register(&hub, "main", None).await;
    let connection = register(&hub, "worker", Some("main")).await;
    let mut events = hub.lock().await.ui.subscribe_events();
    let _loop = start_event_loop(hub.clone(), rx);

    publish(&tx, &mut events, "worker", AgentEventPayload::Running).await;
    publish(
        &tx,
        &mut events,
        "worker",
        AgentEventPayload::Stream {
            text: "final answer".into(),
        },
    )
    .await;
    {
        let mut hub = hub.lock().await;
        hub.registry
            .emit_agent_finished("worker", Some("final answer".into()));
        hub.registry.unregister_connection("worker");
    }
    events.recv().await.expect("Finished broadcast");

    assert!(
        connection.upgrade().is_none(),
        "tombstone retained a connection"
    );
    let (snapshot, topology) = cold_state(hub.clone(), "worker").await;
    assert_eq!(snapshot.rev, 3);
    assert_eq!(
        snapshot.state.agent.observable.status,
        AgentStatus::Finished
    );
    assert!(has_message(&snapshot, "assistant", "final answer"));

    let worker = topology["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|agent| agent["name"] == "worker")
        .unwrap();
    assert_eq!(worker["lifecycle"], "finished");
    assert_eq!(worker["parent"], "main");
    let main = &topology["agents"][0];
    assert_eq!(main["children"], serde_json::json!(["worker"]));
}

#[tokio::test]
async fn failed_child_stays_failed_after_synthetic_finished() {
    let (hub, tx, rx) = make_hub();
    register(&hub, "worker", None).await;
    let mut events = hub.lock().await.ui.subscribe_events();
    let _loop = start_event_loop(hub.clone(), rx);

    publish(&tx, &mut events, "worker", AgentEventPayload::Running).await;
    publish(
        &tx,
        &mut events,
        "worker",
        AgentEventPayload::Error {
            message: "provider failed".into(),
        },
    )
    .await;
    {
        let mut hub = hub.lock().await;
        hub.registry.emit_agent_finished("worker", None);
        hub.registry.unregister_connection("worker");
    }
    events.recv().await.expect("Finished broadcast");

    let (snapshot, topology) = cold_state(hub.clone(), "worker").await;
    assert_eq!(snapshot.rev, 2);
    assert_eq!(snapshot.state.agent.observable.status, AgentStatus::Error);
    let worker = &topology["agents"][0];
    assert_eq!(worker["lifecycle"], "failed");
    assert_eq!(worker["error"], "provider failed");
    assert!(has_message(&snapshot, "error", "provider failed"));
}

#[tokio::test]
async fn completed_bundle_has_a_strict_limit_and_no_connections() {
    let (tx, _rx) = mpsc::channel(8);
    let mut registry = AgentRegistry::new(tx);
    registry.set_completed_agent_limit(2);

    for index in 0..3 {
        let name = format!("worker-{index}");
        let (_peer, transport) = loopal_ipc::duplex_pair();
        let (connection, _incoming) = Connection::new(transport).into_listening();
        registry.register_connection(&name, connection).unwrap();
        registry.set_lifecycle(&name, AgentLifecycle::Running);
        registry.emit_agent_finished(&name, Some(name.clone()));
        registry.unregister_connection(&name);
    }

    assert_eq!(registry.completed_agent_count(), 2);
    assert!(registry.agent_view("worker-0").is_none());
    assert!(registry.completion_output("worker-0").is_none());
    assert!(registry.get_agent_connection("worker-1").is_none());
    assert!(registry.agent_view("worker-1").is_some());
    assert!(registry.agent_view("worker-2").is_some());
    let topology = registry.topology_snapshot();
    assert_eq!(topology["agents"].as_array().unwrap().len(), 2);
}
