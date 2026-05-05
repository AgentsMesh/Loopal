//! End-to-end seed test for the multi-UI attach path.
//!
//! Verifies that a UI joining mid-session can pull a `view/snapshot`
//! containing the full conversation history (messages + tool calls)
//! that earlier events produced — the contract `attach_mode` relies on
//! to populate `app.view_clients` before `run_tui`.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::{Hub, UiSession, start_event_loop};
use loopal_ipc::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress};
use loopal_view_state::{ViewSnapshot, ViewSnapshotRequest};

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

/// Pre-attach traffic must be visible in the snapshot a late-joining UI
/// pulls — otherwise the attach-mode TUI sees an empty conversation.
#[tokio::test]
async fn snapshot_includes_pre_attach_conversation() {
    let (hub, raw_tx, raw_rx) = make_hub();
    register_test_agent(&hub, "worker").await;
    let _handle = start_event_loop(hub.clone(), raw_rx);

    for payload in [
        AgentEventPayload::Stream {
            text: "hello from agent".into(),
        },
        AgentEventPayload::AwaitingInput,
    ] {
        raw_tx.send(named_event("worker", payload)).await.unwrap();
    }
    tokio::time::sleep(Duration::from_millis(50)).await;

    let ui = UiSession::connect(hub.clone(), "late-tui").await;
    let req = ViewSnapshotRequest {
        agent: "worker".into(),
    };
    let resp = ui
        .client
        .connection()
        .send_request(
            methods::VIEW_SNAPSHOT.name,
            serde_json::to_value(req).unwrap(),
        )
        .await
        .expect("snapshot ok");
    let snapshot: ViewSnapshot = serde_json::from_value(resp).expect("parse snapshot");

    let messages = &snapshot.state.agent.conversation.messages;
    assert_eq!(
        messages.len(),
        1,
        "assistant message flushed on AwaitingInput"
    );
    assert_eq!(messages[0].role, "assistant");
    assert!(
        messages[0].content.contains("hello from agent"),
        "snapshot lost streaming content: {:?}",
        messages[0].content
    );
}

/// Tool calls must round-trip through the snapshot too — the attach UI
/// renders historical tool invocations the same way as live ones.
#[tokio::test]
async fn snapshot_includes_completed_tool_call() {
    let (hub, raw_tx, raw_rx) = make_hub();
    register_test_agent(&hub, "worker").await;
    let _handle = start_event_loop(hub.clone(), raw_rx);

    raw_tx
        .send(named_event(
            "worker",
            AgentEventPayload::ToolCall {
                id: "tc-1".into(),
                name: "Read".into(),
                input: serde_json::json!({"file_path": "/tmp/x"}),
            },
        ))
        .await
        .unwrap();
    raw_tx
        .send(named_event(
            "worker",
            AgentEventPayload::ToolResult {
                id: "tc-1".into(),
                name: "Read".into(),
                result: "snapshot payload".into(),
                is_error: false,
                duration_ms: Some(7),
                metadata: None,
            },
        ))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let ui = UiSession::connect(hub.clone(), "late-tui").await;
    let resp = ui
        .client
        .connection()
        .send_request(
            methods::VIEW_SNAPSHOT.name,
            serde_json::json!({"agent": "worker"}),
        )
        .await
        .expect("snapshot ok");
    let snapshot: ViewSnapshot = serde_json::from_value(resp).expect("parse snapshot");

    let tc = &snapshot.state.agent.conversation.messages[0].tool_calls[0];
    assert_eq!(tc.id, "tc-1");
    assert_eq!(tc.name, "Read");
    assert_eq!(tc.duration_ms, Some(7));
    assert_eq!(tc.result.as_deref(), Some("snapshot payload"));
}
