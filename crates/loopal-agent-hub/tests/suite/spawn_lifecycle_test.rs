//! Integration tests for spawn lifecycle using mock transport.
//!
//! Uses `register_agent_connection` with duplex pairs instead of real processes,
//! enabling full spawn/wait/route testing without forking.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::Hub;
use loopal_agent_hub::hub_server;
use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use serde_json::json;

fn make_hub() -> (Arc<Mutex<Hub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(64);
    (Arc::new(Mutex::new(Hub::new(tx))), rx)
}

// ── Spawn + register via mock transport ─────────────────────────────

#[tokio::test]
async fn register_agent_connection_makes_agent_routable() {
    let (hub, mut event_rx) = make_hub();

    // Create mock agent connection (duplex pair)
    let (agent_client, agent_server) = loopal_ipc::duplex_pair();
    let (agent_conn, agent_rx) = Connection::new(agent_client).into_listening();
    let (server_conn, server_rx) = Connection::new(agent_server).into_listening();

    // Register via testable API
    let agent_id = register_agent_connection(
        hub.clone(),
        "mock-worker",
        server_conn,
        server_rx,
        None,
        None,
        None,
    )
    .await
    .unwrap();
    assert!(!agent_id.is_empty());

    // Should receive SubAgentSpawned event
    let event = tokio::time::timeout(Duration::from_secs(1), event_rx.recv()).await;
    assert!(event.is_ok());
    let evt = event.unwrap().unwrap();
    if let AgentEventPayload::SubAgentSpawned(s) = evt.payload {
        assert_eq!(s.name, "mock-worker");
    } else {
        panic!("expected SubAgentSpawned, got {:?}", evt.payload);
    }

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Agent should be registered and routable
    assert!(
        hub.lock()
            .await
            .registry
            .get_agent_connection("mock-worker")
            .is_some()
    );

    // Mock agent responds to requests
    let ac = agent_conn.clone();
    tokio::spawn(async move {
        let mut rx = agent_rx;
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, .. } = msg {
                let _ = ac.respond(id, json!({"received": true})).await;
            }
        }
    });

    // Another agent can route to mock-worker
    let (sender_conn, sr) = hub_server::connect_local(hub.clone(), "sender");
    let sc = sender_conn.clone();
    tokio::spawn(async move {
        let mut rx = sr;
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, .. } = msg {
                let _ = sc.respond(id, json!({"ok": true})).await;
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let envelope = json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "source": {"Agent": {"hub": [], "agent": "sender"}},
        "target": {"hub": [], "agent": "mock-worker"},
        "content": {"text": "hello mock", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = sender_conn
        .send_request(methods::HUB_ROUTE.name, envelope)
        .await;
    assert!(result.is_ok(), "should route to mock agent");
}

#[tokio::test]
async fn register_waits_for_spawn_event_capacity_without_holding_hub_lock() {
    let (event_tx, mut event_rx) = mpsc::channel(1);
    event_tx
        .send(AgentEvent::root(AgentEventPayload::Running))
        .await
        .unwrap();
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let (agent_client, agent_server) = loopal_ipc::duplex_pair();
    let (_agent_conn, _agent_rx) = Connection::new(agent_client).into_listening();
    let (server_conn, server_rx) = Connection::new(agent_server).into_listening();

    let registration = tokio::spawn({
        let hub = hub.clone();
        async move {
            register_agent_connection(
                hub,
                "backpressured-worker",
                server_conn,
                server_rx,
                None,
                None,
                None,
            )
            .await
        }
    });
    tokio::task::yield_now().await;
    assert!(
        !registration.is_finished(),
        "registration must wait until SubAgentSpawned enters the authoritative queue"
    );
    assert!(
        hub.lock()
            .await
            .registry
            .get_agent_connection("backpressured-worker")
            .is_some(),
        "registration state must be committed before event backpressure"
    );
    let guard = tokio::time::timeout(Duration::from_millis(100), hub.lock())
        .await
        .expect("spawn event backpressure must not hold the Hub lock");
    drop(guard);

    assert!(matches!(
        event_rx.recv().await.unwrap().payload,
        AgentEventPayload::Running
    ));
    let spawned = tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
        .await
        .expect("SubAgentSpawned must be delivered when capacity returns")
        .unwrap();
    assert!(matches!(
        spawned.payload,
        AgentEventPayload::SubAgentSpawned(ref spawn) if spawn.name == "backpressured-worker"
    ));
    assert!(!registration.await.unwrap().unwrap().is_empty());
}

#[tokio::test]
async fn buffered_completion_cannot_overtake_sub_agent_spawned() {
    let (hub, mut event_rx) = make_hub();
    let (agent_client, agent_server) = loopal_ipc::duplex_pair();
    let (agent_conn, _agent_rx) = Connection::new(agent_client).into_listening();
    let (server_conn, server_rx) = Connection::new(agent_server).into_listening();

    // Queue completion before registration installs the IO owner. A very short
    // real agent can produce exactly this ordering during process bootstrap.
    agent_conn
        .send_notification(
            methods::AGENT_COMPLETED.name,
            json!({"reason": "error", "result": "failed immediately"}),
        )
        .await
        .unwrap();
    register_agent_connection(
        hub,
        "instant-worker",
        server_conn,
        server_rx,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    let spawned = tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
        .await
        .expect("SubAgentSpawned missing")
        .unwrap();
    assert!(matches!(
        spawned.payload,
        AgentEventPayload::SubAgentSpawned(ref spawn) if spawn.name == "instant-worker"
    ));
    let error = tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
        .await
        .expect("Error missing")
        .unwrap();
    assert!(matches!(error.payload, AgentEventPayload::Error { .. }));
    let finished = tokio::time::timeout(Duration::from_secs(1), event_rx.recv())
        .await
        .expect("Finished missing")
        .unwrap();
    assert!(matches!(finished.payload, AgentEventPayload::Finished));
}

#[tokio::test]
async fn parent_reconnect_during_spawn_backpressure_prevents_orphan_start() {
    let (event_tx, mut event_rx) = mpsc::channel(1);
    event_tx
        .send(AgentEvent::root(AgentEventPayload::Running))
        .await
        .unwrap();
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let (old_parent_peer, old_parent_transport) = loopal_ipc::duplex_pair();
    let (_old_parent, _old_parent_rx) = Connection::new(old_parent_peer).into_listening();
    let (old_parent, old_hub_rx) = Connection::new(old_parent_transport).into_listening();
    let old_dispatcher = Arc::new(loopal_agent_hub::dispatch::build_hub_dispatcher(
        hub.clone(),
    ));
    let (old_ready_tx, old_ready_rx) = tokio::sync::oneshot::channel();
    loopal_agent_hub::agent_io::start_agent_io(
        hub.clone(),
        old_dispatcher,
        "parent",
        old_parent,
        old_hub_rx,
        Some(old_ready_tx),
    );
    old_ready_rx.await.unwrap();
    let (child_peer, child_transport) = loopal_ipc::duplex_pair();
    let (_child, _child_rx) = Connection::new(child_peer).into_listening();
    let (child, child_incoming) = Connection::new(child_transport).into_listening();

    let registration = tokio::spawn({
        let hub = hub.clone();
        async move {
            register_agent_connection(
                hub,
                "stale-parent-child",
                child,
                child_incoming,
                Some("parent"),
                None,
                None,
            )
            .await
        }
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        while hub
            .lock()
            .await
            .registry
            .agent_info("stale-parent-child")
            .is_none()
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let (new_parent_peer, new_parent_transport) = loopal_ipc::duplex_pair();
    let (_new_parent, _new_parent_rx) = Connection::new(new_parent_peer).into_listening();
    let (new_parent, new_hub_rx) = Connection::new(new_parent_transport).into_listening();
    hub.lock().await.registry.unregister_connection("parent");
    let new_dispatcher = Arc::new(loopal_agent_hub::dispatch::build_hub_dispatcher(
        hub.clone(),
    ));
    let (new_ready_tx, new_ready_rx) = tokio::sync::oneshot::channel();
    loopal_agent_hub::agent_io::start_agent_io(
        hub.clone(),
        new_dispatcher,
        "parent",
        new_parent,
        new_hub_rx,
        Some(new_ready_tx),
    );
    new_ready_rx.await.unwrap();

    assert!(matches!(
        event_rx.recv().await.unwrap().payload,
        AgentEventPayload::Running
    ));
    assert!(matches!(
        event_rx.recv().await.unwrap().payload,
        AgentEventPayload::SubAgentSpawned(_)
    ));
    let error = registration.await.unwrap().unwrap_err();
    assert!(error.contains("reconnected before spawn admission"));
    assert!(
        hub.lock()
            .await
            .registry
            .agent_info("stale-parent-child")
            .is_none()
    );
}

#[tokio::test]
async fn closed_spawn_event_queue_unregisters_the_new_agent() {
    let (event_tx, event_rx) = mpsc::channel(1);
    drop(event_rx);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let shutdown = hub.lock().await.shutdown_signal.clone();
    let (agent_client, agent_server) = loopal_ipc::duplex_pair();
    let (_agent_conn, _agent_rx) = Connection::new(agent_client).into_listening();
    let (server_conn, server_rx) = Connection::new(agent_server).into_listening();

    let error = register_agent_connection(
        hub.clone(),
        "unobservable-worker",
        server_conn,
        server_rx,
        None,
        None,
        None,
    )
    .await
    .unwrap_err();
    assert!(error.contains("authoritative Hub event queue closed"));
    assert!(
        hub.lock()
            .await
            .registry
            .get_agent_connection("unobservable-worker")
            .is_none()
    );
    tokio::time::timeout(Duration::from_millis(100), shutdown.notified())
        .await
        .expect("closed spawn event queue must invalidate the Hub");
}

// ── Wait for agent completion ───────────────────────────────────────

#[tokio::test]
async fn wait_agent_returns_when_agent_disconnects() {
    let (hub, _event_rx) = make_hub();

    // Create mock agent
    let (agent_client, agent_server) = loopal_ipc::duplex_pair();
    let (_agent_conn, _agent_rx) = Connection::new(agent_client).into_listening();
    let (server_conn, server_rx) = Connection::new(agent_server).into_listening();

    let _ = register_agent_connection(
        hub.clone(),
        "ephemeral",
        server_conn,
        server_rx,
        None,
        None,
        None,
    )
    .await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Start waiting in background
    let hub_wait = hub.clone();
    let wait_handle = tokio::spawn(async move {
        loopal_agent_hub::dispatch::dispatch_hub_request(
            &hub_wait,
            methods::HUB_WAIT_AGENT.name,
            json!({"name": "ephemeral"}),
            "waiter".into(),
        )
        .await
    });

    // Give wait_agent time to set up watcher
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Simulate agent completion (in production, this happens when stdio closes)
    {
        let mut h = hub.lock().await;
        let _pending = h
            .registry
            .emit_agent_finished("ephemeral", Some("test output".into()));
    }

    // Wait should complete
    let result = tokio::time::timeout(Duration::from_secs(3), wait_handle).await;
    assert!(result.is_ok(), "wait should complete after disconnect");
    let inner = result.unwrap().unwrap();
    assert!(inner.is_ok(), "should return Ok");
}

// ── Spawned agent sends hub/route back to parent ────────────────────

#[tokio::test]
async fn spawned_agent_routes_message_to_parent() {
    let (hub, _event_rx) = make_hub();

    // Parent agent
    let (parent_conn, parent_rx) = hub_server::connect_local(hub.clone(), "parent");
    let (method_tx, mut method_rx) = mpsc::channel::<String>(1);
    let pc = parent_conn.clone();
    tokio::spawn(async move {
        let mut rx = parent_rx;
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, method, .. } = msg {
                let _ = method_tx.send(method).await;
                let _ = pc.respond(id, json!({"ok": true})).await;
            }
        }
    });

    // Mock child agent (registered as if spawned by Hub)
    let (child_client, child_server) = loopal_ipc::duplex_pair();
    let (child_conn, _child_rx) = Connection::new(child_client).into_listening();
    let (server_conn, server_rx) = Connection::new(child_server).into_listening();

    let _ = register_agent_connection(
        hub.clone(),
        "child",
        server_conn,
        server_rx,
        None,
        None,
        None,
    )
    .await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Child sends hub/route targeting parent
    let envelope = json!({
        "id": "00000000-0000-0000-0000-000000000002",
        "source": {"Agent": {"hub": [], "agent": "child"}},
        "target": {"hub": [], "agent": "parent"},
        "content": {"text": "report from child", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = child_conn
        .send_request(methods::HUB_ROUTE.name, envelope)
        .await;
    assert!(result.is_ok(), "child should route to parent via Hub");

    // Parent should receive agent/message
    let method = tokio::time::timeout(Duration::from_secs(2), method_rx.recv()).await;
    assert_eq!(
        method.unwrap().unwrap(),
        methods::AGENT_MESSAGE.name,
        "parent should receive agent/message from child"
    );
}
