//! Integration test: hub event router applies events to per-agent
//! `ViewStateReducer` (used by `view/snapshot`) and forwards each
//! event on the raw `agent/event` broadcast (used by UI clients).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::{AgentLifecycle, Hub, start_event_loop};
use loopal_ipc::Connection;
use loopal_protocol::{
    AgentCompletion, AgentEvent, AgentEventPayload, AgentStatus, QualifiedAddress,
};

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
    let (conn, _rx) = Connection::new(t2).into_listening();
    let mut h = hub.lock().await;
    h.registry
        .register_connection(name, conn)
        .expect("register agent");
    h.registry.set_lifecycle(name, AgentLifecycle::Running);
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
    hub.lock()
        .await
        .registry
        .set_lifecycle("worker", AgentLifecycle::Spawning);
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
    drop(reducer);
    assert_eq!(
        hub.lock()
            .await
            .registry
            .agent_info("worker")
            .unwrap()
            .lifecycle,
        AgentLifecycle::Running
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

#[tokio::test]
async fn error_event_and_typed_failure_keep_topology_failed() {
    let (hub, raw_tx, raw_rx) = make_hub();
    register_test_agent(&hub, "worker").await;
    let mut ui_rx = hub.lock().await.ui.subscribe_events();
    let _handle = start_event_loop(hub.clone(), raw_rx);

    raw_tx
        .send(named_event(
            "worker",
            AgentEventPayload::Error {
                message: "provider failed".into(),
            },
        ))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_millis(200), ui_rx.recv())
        .await
        .expect("error broadcast timeout")
        .expect("error broadcast");

    let mut completion = {
        let mut h = hub.lock().await;
        assert_eq!(
            h.registry.agent_info("worker").unwrap().lifecycle,
            AgentLifecycle::Failed("provider failed".into())
        );
        h.registry.emit_agent_completion(
            "worker",
            AgentCompletion::new("error", Some("provider failed".into())),
        )
    };
    completion.deliver_events().await.unwrap();
    tokio::time::timeout(Duration::from_millis(200), ui_rx.recv())
        .await
        .expect("finished broadcast timeout")
        .expect("finished broadcast");

    let view = {
        let h = hub.lock().await;
        assert_eq!(
            h.registry.agent_info("worker").unwrap().lifecycle,
            AgentLifecycle::Failed("provider failed".into())
        );
        h.registry.agent_view("worker").unwrap()
    };
    assert_eq!(
        view.lock().await.state().agent.observable.status,
        AgentStatus::Error
    );
}
