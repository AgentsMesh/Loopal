//! Tests for the event-driven permission/question lifecycle covering
//! UI deny, no-UI fast deny, timeout, and duplicate tool_call_id.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, broadcast, mpsc};

use loopal_agent_hub::{Hub, UiSession, hub_server, start_event_loop};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use serde_json::json;

fn make_hub() -> (Arc<Mutex<Hub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(64);
    (Arc::new(Mutex::new(Hub::new(tx))), rx)
}

async fn first_permission_event(rx: &mut broadcast::Receiver<AgentEvent>) -> AgentEvent {
    loop {
        let ev = rx.recv().await.expect("event broadcast closed");
        if matches!(ev.payload, AgentEventPayload::ToolPermissionRequest { .. }) {
            return ev;
        }
    }
}

fn key_for(agent: &str, id: &str) -> (String, String) {
    (agent.to_string(), id.to_string())
}

#[tokio::test]
async fn ui_deny_returns_false_to_agent() {
    let (hub, raw_rx) = make_hub();
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    let ui = UiSession::connect(hub.clone(), "ui-1").await;
    let (agent_conn, _) = hub_server::connect_local(hub.clone(), "agent-1");

    let client = ui.client.clone();
    tokio::spawn(async move {
        let mut rx = ui.event_rx;
        let ev = first_permission_event(&mut rx).await;
        if let AgentEventPayload::ToolPermissionRequest { id, .. } = ev.payload {
            let agent = ev.agent_name.unwrap().agent;
            client.respond_permission(&agent, &id, false).await;
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = agent_conn
        .send_request(
            methods::AGENT_PERMISSION.name,
            json!({"tool_call_id": "deny-1", "tool_name": "Bash", "tool_input": {}}),
        )
        .await
        .unwrap();
    assert_eq!(result["allow"], false);
    assert!(
        !hub.lock()
            .await
            .pending_permissions
            .contains_key(&key_for("agent-1", "deny-1"))
    );
}

#[tokio::test]
async fn no_ui_fast_denies_permission() {
    let (hub, _raw_rx) = make_hub();
    let (agent_conn, _) = hub_server::connect_local(hub.clone(), "agent-1");
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        agent_conn.send_request(
            methods::AGENT_PERMISSION.name,
            json!({"tool_call_id": "no-ui", "tool_name": "Bash", "tool_input": {}}),
        ),
    )
    .await
    .expect("must not timeout: no UI should fast-deny");
    assert_eq!(result.unwrap()["allow"], false);
    assert!(
        !hub.lock()
            .await
            .pending_permissions
            .contains_key(&key_for("agent-1", "no-ui"))
    );
}

#[tokio::test]
async fn same_agent_duplicate_tool_call_id_overwrites_pending() {
    let (hub, raw_rx) = make_hub();
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    let ui = UiSession::connect(hub.clone(), "ui-1").await;
    let (agent_conn, _) = hub_server::connect_local(hub.clone(), "agent-1");
    tokio::time::sleep(Duration::from_millis(30)).await;

    let conn1 = agent_conn.clone();
    let req1 = tokio::spawn(async move {
        conn1
            .send_request(
                methods::AGENT_PERMISSION.name,
                json!({"tool_call_id": "dup", "tool_name": "Bash", "tool_input": {"v": 1}}),
            )
            .await
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let conn2 = agent_conn.clone();
    let req2 = tokio::spawn(async move {
        conn2
            .send_request(
                methods::AGENT_PERMISSION.name,
                json!({"tool_call_id": "dup", "tool_name": "Bash", "tool_input": {"v": 2}}),
            )
            .await
    });
    tokio::time::sleep(Duration::from_millis(80)).await;

    ui.client.respond_permission("agent-1", "dup", true).await;

    let result2 = req2.await.unwrap().unwrap();
    assert_eq!(result2["allow"], true);
    drop(req1);
    assert!(
        !hub.lock()
            .await
            .pending_permissions
            .contains_key(&key_for("agent-1", "dup"))
    );
}

#[tokio::test]
async fn cross_agent_same_tool_call_id_isolated() {
    let (hub, raw_rx) = make_hub();
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    let ui = UiSession::connect(hub.clone(), "ui-1").await;
    let (conn_a, _) = hub_server::connect_local(hub.clone(), "agent-a");
    let (conn_b, _) = hub_server::connect_local(hub.clone(), "agent-b");
    tokio::time::sleep(Duration::from_millis(30)).await;

    let req_a = tokio::spawn({
        let c = conn_a.clone();
        async move {
            c.send_request(
                methods::AGENT_PERMISSION.name,
                json!({"tool_call_id": "shared", "tool_name": "Bash", "tool_input": {}}),
            )
            .await
        }
    });
    let req_b = tokio::spawn({
        let c = conn_b.clone();
        async move {
            c.send_request(
                methods::AGENT_PERMISSION.name,
                json!({"tool_call_id": "shared", "tool_name": "Bash", "tool_input": {}}),
            )
            .await
        }
    });
    tokio::time::sleep(Duration::from_millis(80)).await;

    {
        let h = hub.lock().await;
        assert!(
            h.pending_permissions
                .contains_key(&key_for("agent-a", "shared"))
        );
        assert!(
            h.pending_permissions
                .contains_key(&key_for("agent-b", "shared"))
        );
    }

    ui.client
        .respond_permission("agent-a", "shared", true)
        .await;
    ui.client
        .respond_permission("agent-b", "shared", false)
        .await;

    let resp_a = req_a.await.unwrap().unwrap();
    let resp_b = req_b.await.unwrap().unwrap();
    assert_eq!(resp_a["allow"], true);
    assert_eq!(resp_b["allow"], false);
}

#[tokio::test]
async fn no_ui_question_returns_default_answer() {
    let (hub, _raw_rx) = make_hub();
    let (agent_conn, _) = hub_server::connect_local(hub.clone(), "agent-1");
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        agent_conn.send_request(methods::AGENT_QUESTION.name, json!({"questions": []})),
    )
    .await
    .expect("must not timeout")
    .unwrap();
    let answer = result["answers"][0].as_str().unwrap_or("");
    assert!(answer.contains("no UI"));
}
