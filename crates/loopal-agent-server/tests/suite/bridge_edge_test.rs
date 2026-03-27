//! Tests for parent-child agent IPC interaction: edge cases.
//! Permission handling, disconnect, errors, concurrent children.

use std::time::Duration;

use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_test_support::chunks;
use loopal_test_support::scenarios;

use super::bridge_helpers::{
    collect_agent_events, init_and_start_with, start_child_server, T,
};

// ── Tests ────────────────────────────────────────────────────────────

/// Supervised mode: sub-agent Bash tool triggers permission request -> parent denies.
#[tokio::test]
async fn child_permission_request_denied() {
    let calls = vec![
        chunks::tool_turn("tc-bash", "Bash", serde_json::json!({"command": "echo hi"})),
        chunks::text_turn("ok"),
    ];
    let (conn, mut rx, fixture, _join) = start_child_server(calls).await;
    let _sid = init_and_start_with(
        &conn,
        &fixture,
        "run a command",
        serde_json::json!({"permission_mode": "default"}),
    )
    .await;

    let mut events = Vec::new();
    let mut got_permission_request = false;
    let deadline = tokio::time::Instant::now() + T;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(3), rx.recv()).await {
            Ok(Some(Incoming::Request { id, method, .. })) => {
                if method == methods::AGENT_PERMISSION.name {
                    got_permission_request = true;
                    let _ = conn.respond(id, serde_json::json!({"allow": false})).await;
                }
            }
            Ok(Some(Incoming::Notification { method, params })) => {
                if method == methods::AGENT_EVENT.name {
                    if let Ok(ev) = serde_json::from_value::<AgentEvent>(params) {
                        let terminal = matches!(
                            ev.payload,
                            AgentEventPayload::Finished | AgentEventPayload::AwaitingInput
                        );
                        events.push(ev.payload);
                        if terminal {
                            break;
                        }
                    }
                }
            }
            _ => break,
        }
    }

    assert!(got_permission_request, "should receive permission request");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEventPayload::Finished)),
        "should finish after permission denial"
    );
}

/// agent/shutdown after session completes -> server exits cleanly.
#[tokio::test]
async fn shutdown_after_session_completes() {
    let (conn, mut rx, fixture, server_join) =
        start_child_server(scenarios::simple_text("done")).await;
    let _sid =
        init_and_start_with(&conn, &fixture, "quick", serde_json::json!({})).await;

    // Wait for session to finish
    let events = collect_agent_events(&mut rx).await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEventPayload::Finished)),
        "session should finish"
    );

    // Now send shutdown at the top level
    let _ = tokio::time::timeout(
        T,
        conn.send_request(methods::AGENT_SHUTDOWN.name, serde_json::json!({})),
    )
    .await;

    let result = tokio::time::timeout(Duration::from_secs(5), server_join).await;
    assert!(result.is_ok(), "server should exit after shutdown");
}

/// LLM returns error -> child emits Error/RetryError event -> eventually finishes.
#[tokio::test]
async fn child_provider_error_handled() {
    let (conn, mut rx, fixture, _join) =
        start_child_server(scenarios::immediate_error("model overloaded")).await;
    let _sid =
        init_and_start_with(&conn, &fixture, "fail", serde_json::json!({})).await;

    let mut events = Vec::new();
    let deadline = tokio::time::Instant::now() + T;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(5), rx.recv()).await {
            Ok(Some(Incoming::Notification { method, params })) => {
                if method == methods::AGENT_EVENT.name {
                    if let Ok(ev) = serde_json::from_value::<AgentEvent>(params) {
                        let terminal = matches!(
                            ev.payload,
                            AgentEventPayload::Finished | AgentEventPayload::AwaitingInput
                        );
                        events.push(ev.payload);
                        if terminal {
                            break;
                        }
                    }
                }
            }
            _ => break,
        }
    }

    let has_terminal = events.iter().any(|e| {
        matches!(
            e,
            AgentEventPayload::Finished | AgentEventPayload::AwaitingInput
        )
    });
    assert!(
        has_terminal,
        "should terminate even after provider error: {events:?}"
    );
}

/// Two independent sub-agents finish separately.
#[tokio::test]
async fn two_children_finish_independently() {
    let (conn_a, mut rx_a, fix_a, _ja) =
        start_child_server(scenarios::simple_text("from-child-A")).await;
    let (conn_b, mut rx_b, fix_b, _jb) =
        start_child_server(scenarios::simple_text("from-child-B")).await;

    let _sid_a =
        init_and_start_with(&conn_a, &fix_a, "task A", serde_json::json!({})).await;
    let _sid_b =
        init_and_start_with(&conn_b, &fix_b, "task B", serde_json::json!({})).await;

    let (events_a, events_b) = tokio::join!(
        collect_agent_events(&mut rx_a),
        collect_agent_events(&mut rx_b)
    );

    let text_a: String = events_a
        .iter()
        .filter_map(|e| match e {
            AgentEventPayload::Stream { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    let text_b: String = events_b
        .iter()
        .filter_map(|e| match e {
            AgentEventPayload::Stream { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();

    assert!(text_a.contains("from-child-A"), "child A text");
    assert!(text_b.contains("from-child-B"), "child B text");
    assert!(events_a.iter().any(|e| matches!(e, AgentEventPayload::Finished)));
    assert!(events_b.iter().any(|e| matches!(e, AgentEventPayload::Finished)));
}
