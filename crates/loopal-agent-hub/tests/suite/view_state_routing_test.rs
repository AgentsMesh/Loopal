//! Integration test: hub event router applies events to per-agent
//! `ViewStateReducer` (used by `view/snapshot`) and forwards each
//! event on the raw `agent/event` broadcast (used by UI clients).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::{Hub, start_event_loop};
use loopal_ipc::Connection;
use loopal_protocol::{AgentEvent, AgentEventPayload, AgentStatus, QualifiedAddress};

fn make_hub() -> (
    Arc<Mutex<Hub>>,
    mpsc::Sender<AgentEvent>,
    mpsc::Receiver<AgentEvent>,
) {
    let (raw_tx, raw_rx) = mpsc::channel(16);
    let hub = Arc::new(Mutex::new(Hub::new(raw_tx.clone())));
    (hub, raw_tx, raw_rx)
}

async fn register_test_agent(hub: &Arc<Mutex<Hub>>, name: &str) {
    let (_t1, t2) = loopal_ipc::duplex_pair();
    let conn = Arc::new(Connection::new(t2));
    let _rx = conn.start();
    hub.lock()
        .await
        .registry
        .register_connection(name, conn)
        .expect("register agent");
}

fn named_event(agent: &str, payload: AgentEventPayload) -> AgentEvent {
    AgentEvent::named(QualifiedAddress::local(agent), payload)
}

/// Observable event routed through the hub bumps the agent's reducer rev.
/// `view/snapshot` will reflect the post-event state.
#[tokio::test]
async fn observable_event_updates_hub_reducer() {
    let (hub, raw_tx, raw_rx) = make_hub();
    register_test_agent(&hub, "worker").await;
    let _handle = start_event_loop(hub.clone(), raw_rx);

    raw_tx
        .send(named_event("worker", AgentEventPayload::Running))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let view = {
        let h = hub.lock().await;
        h.registry.agent_view("worker").expect("registered")
    };
    let reducer = view.lock().await;
    assert_eq!(reducer.rev(), 1);
    assert_eq!(
        reducer.state().agent.observable.status,
        AgentStatus::Running
    );
}

/// Non-observable events (Stream, ToolProgress, ...) do not bump rev,
/// but are still forwarded on the raw `agent/event` broadcast.
#[tokio::test]
async fn non_observable_event_is_broadcast_to_ui() {
    let (hub, raw_tx, raw_rx) = make_hub();
    register_test_agent(&hub, "worker").await;
    let mut ui_rx = hub.lock().await.ui.subscribe_events();
    let _handle = start_event_loop(hub.clone(), raw_rx);

    raw_tx
        .send(named_event(
            "worker",
            AgentEventPayload::TurnDiffSummary {
                modified_files: vec![],
            },
        ))
        .await
        .unwrap();

    let received = tokio::time::timeout(Duration::from_millis(200), ui_rx.recv())
        .await
        .expect("timeout")
        .expect("recv");
    assert!(matches!(
        received.payload,
        AgentEventPayload::TurnDiffSummary { .. }
    ));

    let view = {
        let h = hub.lock().await;
        h.registry.agent_view("worker").expect("registered")
    };
    let reducer = view.lock().await;
    assert_eq!(reducer.rev(), 0);
}

/// Events for an agent that isn't registered are silently dropped from
/// the ViewState path; the raw broadcast still reaches UI subscribers.
#[tokio::test]
async fn event_for_unknown_agent_is_silently_ignored() {
    let (hub, raw_tx, raw_rx) = make_hub();
    let mut ui_rx = hub.lock().await.ui.subscribe_events();

    let _handle = start_event_loop(hub, raw_rx);

    raw_tx
        .send(named_event("ghost", AgentEventPayload::Running))
        .await
        .unwrap();

    let received = tokio::time::timeout(Duration::from_millis(200), ui_rx.recv())
        .await
        .expect("timeout")
        .expect("recv");
    assert!(matches!(received.payload, AgentEventPayload::Running));
}
