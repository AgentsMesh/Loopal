//! Tests for the event-driven permission/question flow.
//!
//! agent IPC `agent/permission` → Hub stores pending + emits
//! `ToolPermissionRequest` event → UI responds via `hub/permission_response`
//! → Hub resolves pending and replies to agent.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, broadcast, mpsc};

use loopal_agent_hub::{Hub, HubClient, UiSession, hub_server, start_event_loop};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use serde_json::json;

fn make_hub() -> (Arc<Mutex<Hub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(16);
    (Arc::new(Mutex::new(Hub::new(tx))), rx)
}

async fn approve_via_events(
    mut rx: broadcast::Receiver<AgentEvent>,
    client: Arc<HubClient>,
    allow: bool,
) {
    while let Ok(event) = rx.recv().await {
        if let AgentEventPayload::ToolPermissionRequest { id, .. } = event.payload {
            let agent = event
                .agent_name
                .as_ref()
                .map(|q| q.agent.clone())
                .unwrap_or_else(|| "main".to_string());
            client.respond_permission(&agent, &id, allow).await;
            return;
        }
    }
}

async fn answer_via_events(
    mut rx: broadcast::Receiver<AgentEvent>,
    client: Arc<HubClient>,
    answers: Vec<String>,
) {
    while let Ok(event) = rx.recv().await {
        if let AgentEventPayload::UserQuestionRequest { id, .. } = event.payload {
            let agent = event
                .agent_name
                .as_ref()
                .map(|q| q.agent.clone())
                .unwrap_or_else(|| "main".to_string());
            client.respond_question(&agent, &id, answers).await;
            return;
        }
    }
}

#[tokio::test]
async fn permission_resolved_by_ui_response() {
    let (hub, raw_rx) = make_hub();
    let _event_loop = start_event_loop(hub.clone(), raw_rx);

    let ui = UiSession::connect(hub.clone(), "ui-1").await;
    let (agent_conn, _) = hub_server::connect_local(hub.clone(), "agent-1");

    tokio::spawn(approve_via_events(ui.event_rx, ui.client, true));
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = agent_conn
        .send_request(
            methods::AGENT_PERMISSION.name,
            json!({"tool_call_id": "t1", "tool_name": "Bash", "tool_input": {}}),
        )
        .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap()["allow"], true);
}

#[tokio::test]
async fn question_resolved_by_ui_response() {
    let (hub, raw_rx) = make_hub();
    let _event_loop = start_event_loop(hub.clone(), raw_rx);

    let ui = UiSession::connect(hub.clone(), "ui-1").await;
    let (agent_conn, _) = hub_server::connect_local(hub.clone(), "agent-1");

    tokio::spawn(answer_via_events(
        ui.event_rx,
        ui.client,
        vec!["yes".into()],
    ));
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = agent_conn
        .send_request(methods::AGENT_QUESTION.name, json!({"questions": []}))
        .await;

    assert!(result.is_ok());
    let resp = result.unwrap();
    assert_eq!(resp["kind"], "answered");
    assert_eq!(resp["answers"][0], "yes");
}

#[tokio::test]
async fn pending_recorded_then_emitted_event() {
    let (hub, raw_rx) = make_hub();
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    let ui = UiSession::connect(hub.clone(), "ui-1").await;
    let (agent_conn, _) = hub_server::connect_local(hub.clone(), "agent-1");

    tokio::time::sleep(Duration::from_millis(30)).await;
    let req_handle = tokio::spawn(async move {
        agent_conn
            .send_request(
                methods::AGENT_PERMISSION.name,
                json!({"tool_call_id": "tc-7", "tool_name": "Bash", "tool_input": {}}),
            )
            .await
    });

    let mut event_rx = ui.event_rx;
    let event = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(ev) = event_rx.recv().await
                && matches!(ev.payload, AgentEventPayload::ToolPermissionRequest { .. })
            {
                return ev;
            }
        }
    })
    .await
    .expect("timeout waiting for ToolPermissionRequest");

    assert!(matches!(
        event.payload,
        AgentEventPayload::ToolPermissionRequest { ref id, .. } if id == "tc-7"
    ));
    assert!(
        hub.lock()
            .await
            .pending_permissions
            .contains_key(&("agent-1".to_string(), "tc-7".to_string()))
    );

    ui.client.respond_permission("agent-1", "tc-7", false).await;
    let resp = req_handle.await.expect("agent task panicked");
    assert!(resp.is_ok());
    assert_eq!(resp.unwrap()["allow"], false);
    assert!(
        !hub.lock()
            .await
            .pending_permissions
            .contains_key(&("agent-1".to_string(), "tc-7".to_string()))
    );
}

#[tokio::test]
async fn ui_client_lifecycle() {
    let (hub, _event_rx) = make_hub();

    assert!(!hub.lock().await.ui.is_ui_client("my-ui"));

    let _ui = UiSession::connect(hub.clone(), "my-ui").await;
    assert!(hub.lock().await.ui.is_ui_client("my-ui"));

    hub.lock().await.ui.unregister_client("my-ui");
    assert!(!hub.lock().await.ui.is_ui_client("my-ui"));
}
