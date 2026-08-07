//! Tests for the hub event router.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_agent_hub::{AgentLifecycle, Hub, start_event_loop};
use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentCompletion, AgentEvent, AgentEventPayload, AgentStatus};

fn make_hub_and_channels() -> (
    Arc<Mutex<Hub>>,
    mpsc::Sender<AgentEvent>,
    mpsc::Receiver<AgentEvent>,
) {
    let (raw_tx, raw_rx) = mpsc::channel(16);
    let hub = Arc::new(Mutex::new(Hub::new(raw_tx.clone())));
    (hub, raw_tx, raw_rx)
}

async fn register_mock_agent(hub: &Arc<Mutex<Hub>>, name: &str) -> Arc<Connection<Listening>> {
    let (agent_transport, hub_transport) = loopal_ipc::duplex_pair();
    let (agent, _agent_rx) = Connection::new(agent_transport).into_listening();
    let (server, server_rx) = Connection::new(hub_transport).into_listening();
    register_agent_connection(hub.clone(), name, server, server_rx, None, None, None)
        .await
        .unwrap();
    agent
}

async fn send_event(agent: &Connection<Listening>, name: &str, payload: AgentEventPayload) {
    let event = AgentEvent::named(name, payload);
    agent
        .send_notification(
            methods::AGENT_EVENT.name,
            serde_json::to_value(event).unwrap(),
        )
        .await
        .unwrap();
}

async fn wait_for_completion(hub: &Arc<Mutex<Hub>>, name: &str) -> AgentCompletion {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(completion) = hub.lock().await.registry.completion(name).cloned() {
                return completion;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("agent completion timeout")
}

async fn wait_for_barrier(
    events: &mut tokio::sync::broadcast::Receiver<AgentEvent>,
    message: &str,
) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if matches!(
                events.recv().await.unwrap().payload,
                AgentEventPayload::ProviderWarning { message: current } if current == message
            ) {
                return;
            }
        }
    })
    .await
    .expect("event router barrier timeout");
}

fn lifecycle_for(topology: &serde_json::Value, name: &str) -> String {
    topology["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|agent| agent["name"] == name)
        .unwrap()["lifecycle"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Normal events are forwarded via broadcast.
#[tokio::test]
async fn forwards_events_to_subscriber() {
    let (hub, raw_tx, raw_rx) = make_hub_and_channels();
    let mut sub_rx = hub.lock().await.ui.subscribe_events();

    let _handle = start_event_loop(hub, raw_rx);

    let event = AgentEvent::root(AgentEventPayload::Stream {
        text: "hello".into(),
    });
    raw_tx.send(event).await.unwrap();

    let received = tokio::time::timeout(Duration::from_millis(100), sub_rx.recv())
        .await
        .expect("timeout")
        .expect("recv error");

    assert!(matches!(received.payload, AgentEventPayload::Stream { .. }));
}

/// Multiple events arrive in order.
#[tokio::test]
async fn preserves_event_order() {
    let (hub, raw_tx, raw_rx) = make_hub_and_channels();
    let mut sub_rx = hub.lock().await.ui.subscribe_events();

    let _handle = start_event_loop(hub, raw_rx);

    for i in 0..5 {
        let event = AgentEvent::root(AgentEventPayload::Stream {
            text: format!("msg-{i}"),
        });
        raw_tx.send(event).await.unwrap();
    }

    for i in 0..5 {
        let received = tokio::time::timeout(Duration::from_millis(100), sub_rx.recv())
            .await
            .expect("timeout")
            .expect("recv error");
        if let AgentEventPayload::Stream { text } = received.payload {
            assert_eq!(text, format!("msg-{i}"));
        } else {
            panic!("unexpected payload");
        }
    }
}

/// Loop exits when raw_rx is closed (all senders dropped).
#[tokio::test]
async fn exits_on_raw_channel_close() {
    let hub = Arc::new(Mutex::new(Hub::noop()));
    let (raw_tx, raw_rx) = mpsc::channel::<AgentEvent>(16);

    let handle = start_event_loop(hub, raw_rx);
    drop(raw_tx);

    tokio::time::timeout(Duration::from_millis(200), handle)
        .await
        .expect("event loop should exit when raw channel closes")
        .expect("task should not panic");
}

/// Multiple subscribers each receive the same events.
#[tokio::test]
async fn multiple_subscribers_receive_events() {
    let (hub, raw_tx, raw_rx) = make_hub_and_channels();
    let mut sub1 = hub.lock().await.ui.subscribe_events();
    let mut sub2 = hub.lock().await.ui.subscribe_events();

    let _handle = start_event_loop(hub, raw_rx);

    let event = AgentEvent::root(AgentEventPayload::Stream {
        text: "broadcast".into(),
    });
    raw_tx.send(event).await.unwrap();

    let r1 = tokio::time::timeout(Duration::from_millis(100), sub1.recv())
        .await
        .expect("timeout")
        .expect("recv error");
    let r2 = tokio::time::timeout(Duration::from_millis(100), sub2.recv())
        .await
        .expect("timeout")
        .expect("recv error");

    assert!(matches!(r1.payload, AgentEventPayload::Stream { .. }));
    assert!(matches!(r2.payload, AgentEventPayload::Stream { .. }));
}

#[tokio::test]
async fn typed_completion_is_authoritative_over_queued_lifecycle_events() {
    let (hub, raw_tx, raw_rx) = make_hub_and_channels();
    let agent = register_mock_agent(&hub, "worker").await;

    // Keep the reducer stopped until the complete Error -> Running -> goal
    // sequence is queued. This deterministically exercises the old race where
    // Error changed topology synchronously but Running was still in the queue.
    send_event(
        &agent,
        "worker",
        AgentEventPayload::Error {
            message: "old attempt failed".into(),
        },
    )
    .await;
    send_event(&agent, "worker", AgentEventPayload::Running).await;
    agent
        .send_notification(
            methods::AGENT_COMPLETED.name,
            serde_json::json!({"reason": "goal", "result": "recovered"}),
        )
        .await
        .unwrap();

    let completion = wait_for_completion(&hub, "worker").await;
    assert!(completion.is_success());
    assert_eq!(
        lifecycle_for(&hub.lock().await.registry.topology_snapshot(), "worker"),
        "finished"
    );

    let view = hub
        .lock()
        .await
        .registry
        .agent_view("worker")
        .expect("terminal view");
    let mut events = hub.lock().await.ui.subscribe_events();
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    raw_tx
        .send(AgentEvent::root(AgentEventPayload::ProviderWarning {
            message: "completion-order-barrier".into(),
        }))
        .await
        .unwrap();
    wait_for_barrier(&mut events, "completion-order-barrier").await;

    assert_eq!(
        view.lock().await.state().agent.observable.status,
        AgentStatus::Finished
    );
    assert_eq!(
        lifecycle_for(&hub.lock().await.registry.topology_snapshot(), "worker"),
        "finished",
        "queued Running must not reopen an authoritative completion"
    );
}

#[tokio::test]
async fn queued_old_generation_events_do_not_reach_same_name_reconnect() {
    let (hub, raw_tx, raw_rx) = make_hub_and_channels();
    let old_agent = register_mock_agent(&hub, "worker").await;
    send_event(
        &old_agent,
        "worker",
        AgentEventPayload::Error {
            message: "old generation fatal".into(),
        },
    )
    .await;
    old_agent
        .send_notification(
            methods::AGENT_COMPLETED.name,
            serde_json::json!({"reason": "goal", "result": "old result"}),
        )
        .await
        .unwrap();
    wait_for_completion(&hub, "worker").await;

    let _new_agent = register_mock_agent(&hub, "worker").await;
    let new_view = hub
        .lock()
        .await
        .registry
        .agent_view("worker")
        .expect("new generation view");
    let mut events = hub.lock().await.ui.subscribe_events();
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    raw_tx
        .send(AgentEvent::root(AgentEventPayload::ProviderWarning {
            message: "reconnect-backlog-barrier".into(),
        }))
        .await
        .unwrap();
    wait_for_barrier(&mut events, "reconnect-backlog-barrier").await;

    let state = new_view.lock().await.state().clone();
    assert_eq!(state.agent.observable.status, AgentStatus::Starting);
    assert!(
        !state
            .agent
            .conversation
            .messages
            .iter()
            .any(|message| message.content.contains("old generation fatal"))
    );
    assert_eq!(
        lifecycle_for(&hub.lock().await.registry.topology_snapshot(), "worker"),
        "running"
    );
}

#[tokio::test]
async fn late_event_from_detached_connection_cannot_mutate_reconnect() {
    let (hub, raw_tx, raw_rx) = make_hub_and_channels();
    let old_agent = register_mock_agent(&hub, "worker").await;
    hub.lock().await.registry.unregister_connection("worker");
    let _new_agent = register_mock_agent(&hub, "worker").await;
    let new_view = hub
        .lock()
        .await
        .registry
        .agent_view("worker")
        .expect("new generation view");
    let mut events = hub.lock().await.ui.subscribe_events();
    let _event_loop = start_event_loop(hub.clone(), raw_rx);

    send_event(
        &old_agent,
        "worker",
        AgentEventPayload::Error {
            message: "late old connection error".into(),
        },
    )
    .await;
    old_agent
        .send_notification(
            methods::AGENT_COMPLETED.name,
            serde_json::json!({"reason": "goal", "result": "stale"}),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while old_agent.is_connected() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("stale IO loop did not terminate");
    raw_tx
        .send(AgentEvent::root(AgentEventPayload::ProviderWarning {
            message: "late-event-barrier".into(),
        }))
        .await
        .unwrap();
    wait_for_barrier(&mut events, "late-event-barrier").await;

    assert_eq!(
        new_view.lock().await.state().agent.observable.status,
        AgentStatus::Starting
    );
    assert_eq!(
        lifecycle_for(&hub.lock().await.registry.topology_snapshot(), "worker"),
        AgentLifecycle::Running.state()
    );
}
