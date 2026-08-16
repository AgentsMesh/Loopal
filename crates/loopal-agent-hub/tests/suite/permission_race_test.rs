//! Tests for race / crash / timeout scenarios in the event-driven
//! permission lifecycle.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, broadcast, mpsc};

use loopal_agent_hub::{Hub, UiSession, hub_server, start_event_loop};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use serde_json::json;

fn make_hub() -> (Arc<Mutex<Hub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(64);
    (crate::permission_support::hub_with_noop_audit(tx), rx)
}

async fn pending_has(hub: &Arc<Mutex<Hub>>, agent: &str, id: &str) -> bool {
    hub.lock()
        .await
        .pending_permissions
        .contains_key(&(agent.to_string(), id.to_string()))
}

async fn next_resolved(rx: &mut broadcast::Receiver<AgentEvent>) -> Option<String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
            Ok(Ok(ev)) => {
                if let AgentEventPayload::ToolPermissionResolved { id } = ev.payload {
                    return Some(id);
                }
            }
            _ => continue,
        }
    }
    None
}

#[tokio::test]
async fn agent_finish_cleans_pending_permission_and_emits_resolved() {
    let (hub, raw_rx) = make_hub();
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    let ui = UiSession::connect(hub.clone(), "ui-1", loopal_protocol::UiCapabilities::ALL).await;
    let (agent_conn, _) = hub_server::connect_local(hub.clone(), "agent-1");
    tokio::time::sleep(Duration::from_millis(30)).await;

    // Agent fires the request but never responds — UI just observes.
    let req_handle = tokio::spawn({
        let agent_conn = agent_conn.clone();
        async move {
            agent_conn
                .send_request(
                    methods::AGENT_PERMISSION.name,
                    crate::permission_request("tc-stranded", "Bash", json!({})),
                )
                .await
        }
    });

    // Wait for pending to land.
    let mut deadline = tokio::time::Instant::now() + Duration::from_secs(1);
    while tokio::time::Instant::now() < deadline
        && !pending_has(&hub, "agent-1", "tc-stranded").await
    {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(pending_has(&hub, "agent-1", "tc-stranded").await);
    let interaction_id = crate::permission_interaction_id(&hub, "agent-1", "tc-stranded").await;

    // Simulate agent crash: tear down its IO loop.
    let hub_side_conn = hub
        .lock()
        .await
        .registry
        .get_agent_connection("agent-1")
        .unwrap();
    loopal_agent_hub::finish::finish_and_deliver(
        &hub,
        "agent-1",
        loopal_protocol::AgentCompletion::new(
            loopal_protocol::TRANSPORT_ERROR_REASON,
            Some("test transport closed".into()),
        ),
        &hub_side_conn,
    )
    .await;

    // Pending must be gone.
    assert!(!pending_has(&hub, "agent-1", "tc-stranded").await);

    // UI must observe ToolPermissionResolved (the cleanup-emitted one).
    let mut event_rx = ui.event_rx;
    let resolved = next_resolved(&mut event_rx).await;
    assert_eq!(resolved.as_deref(), Some(interaction_id.as_str()));

    // Original IPC request returns an error or a deny — either is acceptable
    // because we close the conn during finish_and_deliver. We just shouldn't hang.
    deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while !req_handle.is_finished() && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        req_handle.is_finished(),
        "agent request must not hang after finish"
    );
}

#[tokio::test]
async fn ui_response_consumes_pending_exactly_once() {
    let (hub, raw_rx) = make_hub();
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    let ui = UiSession::connect(hub.clone(), "ui-1", loopal_protocol::UiCapabilities::ALL).await;
    let (agent_conn, _) = hub_server::connect_local(hub.clone(), "agent-1");
    tokio::time::sleep(Duration::from_millis(30)).await;

    let req_handle = tokio::spawn({
        let agent_conn = agent_conn.clone();
        async move {
            agent_conn
                .send_request(
                    methods::AGENT_PERMISSION.name,
                    crate::permission_request("tc-race", "Bash", json!({})),
                )
                .await
        }
    });
    let interaction = crate::permission_interaction(&hub, "agent-1", "tc-race").await;
    ui.client
        .respond_permission("agent-1", &interaction.id, Some(interaction.digest), true)
        .await;

    let resp = req_handle.await.unwrap().unwrap();
    assert_eq!(resp["allow"], true);
    assert!(!pending_has(&hub, "agent-1", "tc-race").await);

    // A second respond on the same key is a no-op (pending consumed).
    ui.client
        .respond_permission("agent-1", &interaction.id, Some(interaction.digest), false)
        .await;
}

#[tokio::test]
async fn second_ui_respond_after_first_returns_unresolved() {
    let (hub, raw_rx) = make_hub();
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    let ui_a = UiSession::connect(hub.clone(), "ui-a", loopal_protocol::UiCapabilities::ALL).await;
    let ui_b = UiSession::connect(hub.clone(), "ui-b", loopal_protocol::UiCapabilities::ALL).await;
    let (agent_conn, _) = hub_server::connect_local(hub.clone(), "agent-1");
    tokio::time::sleep(Duration::from_millis(50)).await;

    let req_handle = tokio::spawn({
        let agent_conn = agent_conn.clone();
        async move {
            agent_conn
                .send_request(
                    methods::AGENT_PERMISSION.name,
                    crate::permission_request("tc-double", "Bash", json!({})),
                )
                .await
        }
    });
    let interaction = crate::permission_interaction(&hub, "agent-1", "tc-double").await;

    ui_a.client
        .respond_permission("agent-1", &interaction.id, Some(interaction.digest), true)
        .await;
    let resp = req_handle.await.unwrap().unwrap();
    assert_eq!(resp["allow"], true);

    // UI B's respond arrives after pending is gone — must not panic, returns resolved:false.
    ui_b.client
        .respond_permission("agent-1", &interaction.id, Some(interaction.digest), false)
        .await;

    // Both UIs see exactly one Resolved event for tc-double.
    let mut rx_b = ui_b.event_rx;
    assert_eq!(
        next_resolved(&mut rx_b).await.as_deref(),
        Some(interaction.id.as_str())
    );
    // No second Resolved should appear within a short window.
    let second = tokio::time::timeout(Duration::from_millis(200), next_resolved(&mut rx_b)).await;
    assert!(
        second.is_err() || second.unwrap().is_none(),
        "no second Resolved event expected"
    );
}

#[tokio::test]
async fn full_event_queue_backpressures_permission_without_holding_hub_lock_or_denying() {
    let (tx, mut rx) = mpsc::channel::<AgentEvent>(1);
    let hub = crate::permission_support::hub_with_noop_audit(tx);
    let ui = UiSession::connect(hub.clone(), "ui-1", loopal_protocol::UiCapabilities::ALL).await;
    let (agent_conn, _) = hub_server::connect_local(hub.clone(), "agent-1");
    tokio::time::sleep(Duration::from_millis(30)).await;
    hub.lock()
        .await
        .registry
        .event_sender()
        .send(AgentEvent::root(AgentEventPayload::Running))
        .await
        .unwrap();

    let request = tokio::spawn({
        let agent_conn = agent_conn.clone();
        async move {
            agent_conn
                .send_request(
                    methods::AGENT_PERMISSION.name,
                    crate::permission_request("tc-pressure", "Bash", json!({})),
                )
                .await
        }
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        while !pending_has(&hub, "agent-1", "tc-pressure").await {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("permission admission must commit pending state before backpressure");
    assert!(
        !request.is_finished(),
        "a full event queue must backpressure instead of fast-denying"
    );
    let guard = tokio::time::timeout(Duration::from_millis(100), hub.lock())
        .await
        .expect("permission backpressure must not hold the Hub lock");
    drop(guard);

    assert!(matches!(
        rx.recv().await.unwrap().payload,
        AgentEventPayload::Running
    ));
    let permission = tokio::time::timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("permission event must enter the queue once capacity is released")
        .unwrap();
    let AgentEventPayload::ToolPermissionRequest {
        id,
        permission_intent,
        ..
    } = permission.payload
    else {
        panic!("expected ToolPermissionRequest");
    };
    let digest = permission_intent.map(|intent| intent.intent_digest());
    assert!(permission.routing_generation.is_some());

    ui.client
        .respond_permission("agent-1", &id, digest, true)
        .await;
    let result = tokio::time::timeout(Duration::from_secs(2), request)
        .await
        .expect("permission response must resolve after backpressure")
        .unwrap()
        .unwrap();
    assert_eq!(result["allow"], true);
    assert!(!pending_has(&hub, "agent-1", "tc-pressure").await);
}
