//! Tests for the deterministic result passing chain:
//! agent/completed notification carries result → Hub extracts it → wait_agent returns it.
//!
//! This validates the single-path (no fallback) result transmission design.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::Hub;
use loopal_agent_hub::agent_io::agent_io_loop;
use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEvent;
use serde_json::json;

type IoHandle = tokio::task::JoinHandle<Option<String>>;

/// Set up a Hub + duplex connection + spawn agent_io_loop. Returns the
/// client-side connection (to send notifications) and the io_loop join handle.
async fn setup_agent(name: &str) -> (Arc<Connection>, IoHandle) {
    let (tx, _rx) = mpsc::channel::<AgentEvent>(64);
    let hub = Arc::new(Mutex::new(Hub::new(tx)));
    let (agent_side, hub_side) = loopal_ipc::duplex_pair();
    let agent_conn = Arc::new(Connection::new(agent_side));
    let hub_conn = Arc::new(Connection::new(hub_side));
    let _agent_rx = agent_conn.start();
    let hub_rx = hub_conn.start();
    let hc = hub_conn.clone();
    let n = name.to_string();
    let handle = tokio::spawn(async move { agent_io_loop(hub, hc, hub_rx, n).await });
    (agent_conn, handle)
}

/// agent/completed with result field → agent_io_loop returns that result.
#[tokio::test]
async fn agent_completed_with_result() {
    let (agent_conn, io_handle) = setup_agent("worker").await;

    // Agent sends agent/completed with result
    agent_conn
        .send_notification(
            methods::AGENT_COMPLETED.name,
            json!({"reason": "goal", "result": "Found 42 issues."}),
        )
        .await
        .unwrap();

    let result = tokio::time::timeout(Duration::from_secs(3), io_handle).await;
    let output = result.unwrap().unwrap();
    assert_eq!(output.as_deref(), Some("Found 42 issues."));
}

/// agent/completed without result field → agent_io_loop returns None.
#[tokio::test]
async fn agent_completed_without_result() {
    let (agent_conn, io_handle) = setup_agent("worker2").await;

    // Legacy notification without result
    agent_conn
        .send_notification(methods::AGENT_COMPLETED.name, json!({"reason": "shutdown"}))
        .await
        .unwrap();

    let result = tokio::time::timeout(Duration::from_secs(3), io_handle).await;
    let output = result.unwrap().unwrap();
    assert_eq!(output, None, "no result field → None");
}

/// Stream events are forwarded to UI but NOT used as output source.
#[tokio::test]
async fn stream_events_not_used_as_output() {
    let (agent_conn, io_handle) = setup_agent("worker3").await;

    // Agent streams text (should NOT be captured as output)
    let stream_event = AgentEvent::named(
        "worker3",
        loopal_protocol::AgentEventPayload::Stream {
            text: "I'm exploring the codebase...".into(),
        },
    );
    agent_conn
        .send_notification(
            methods::AGENT_EVENT.name,
            serde_json::to_value(&stream_event).unwrap(),
        )
        .await
        .unwrap();

    // Agent completes WITHOUT result
    agent_conn
        .send_notification(methods::AGENT_COMPLETED.name, json!({"reason": "shutdown"}))
        .await
        .unwrap();

    let result = tokio::time::timeout(Duration::from_secs(3), io_handle).await;
    let output = result.unwrap().unwrap();
    // Old behavior: would have returned "I'm exploring the codebase..."
    // New behavior: returns None (stream text is not an output source)
    assert_eq!(output, None, "stream text should NOT be used as output");
}

/// agent/completed result field takes precedence even when stream was active.
#[tokio::test]
async fn result_field_overrides_stream_text() {
    let (agent_conn, io_handle) = setup_agent("worker4").await;

    // Agent streams intermediate text
    let stream_event = AgentEvent::named(
        "worker4",
        loopal_protocol::AgentEventPayload::Stream {
            text: "Let me explore...".into(),
        },
    );
    agent_conn
        .send_notification(
            methods::AGENT_EVENT.name,
            serde_json::to_value(&stream_event).unwrap(),
        )
        .await
        .unwrap();

    // Agent completes WITH authoritative result
    agent_conn
        .send_notification(
            methods::AGENT_COMPLETED.name,
            json!({"reason": "goal", "result": "Comprehensive analysis report."}),
        )
        .await
        .unwrap();

    let result = tokio::time::timeout(Duration::from_secs(3), io_handle).await;
    let output = result.unwrap().unwrap();
    assert_eq!(
        output.as_deref(),
        Some("Comprehensive analysis report."),
        "should use result from agent/completed, not stream text"
    );
}

/// agent/completed with error reason still carries result text.
#[tokio::test]
async fn error_reason_with_partial_result() {
    let (agent_conn, io_handle) = setup_agent("errored").await;

    agent_conn
        .send_notification(
            methods::AGENT_COMPLETED.name,
            json!({"reason": "error", "result": "Partial findings before crash"}),
        )
        .await
        .unwrap();

    let result = tokio::time::timeout(Duration::from_secs(3), io_handle).await;
    let output = result.unwrap().unwrap();
    assert_eq!(
        output.as_deref(),
        Some("Partial findings before crash"),
        "error reason should still carry partial result"
    );
}
