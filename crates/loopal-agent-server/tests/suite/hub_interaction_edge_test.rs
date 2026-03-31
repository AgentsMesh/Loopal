//! Edge-case HubFrontend interaction tests: error recovery, permissions,
//! session lifecycle, and extended multi-turn through real agent loop.

use std::time::Duration;

use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEventPayload;
use loopal_test_support::{chunks, scenarios};

use crate::hub_harness::{build_hub_harness, build_hub_harness_with, has_stream};

/// Path 3: LLM stream error → Error event → user sends new message → success.
#[tokio::test]
async fn hub_provider_error_then_new_message_succeeds() {
    let calls = vec![
        vec![chunks::provider_error("502 Bad Gateway")],
        chunks::text_turn("recovered"),
    ];
    let mut h = build_hub_harness(calls).await;

    h.send_message("first attempt").await;
    let ev1 = h.collect_events().await;
    assert!(
        ev1.iter()
            .any(|e| matches!(e, AgentEventPayload::Error { .. })),
        "should emit error event"
    );

    h.send_message("try again").await;
    let ev2 = h.collect_events().await;
    assert!(has_stream(&ev2, "recovered"), "should succeed on retry");
}

/// Path 5: Supervised mode → Bash tool → permission denied via IPC →
/// LLM receives denied result → responds with adjusted text.
#[tokio::test]
async fn hub_permission_denied_then_llm_adjusts() {
    let calls = vec![
        chunks::tool_turn("tc-1", "Bash", serde_json::json!({"command": "echo hi"})),
        chunks::text_turn("adjusted approach"),
    ];
    let mut h =
        build_hub_harness_with(calls, Some(loopal_tool_api::PermissionMode::Supervised)).await;
    h.wait_ready().await;
    h.send_message("run a command").await;

    // Read from IPC connection — skip event notifications, wait for permission request
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut permission_received = false;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(5), h.client_rx.recv()).await {
            Ok(Some(Incoming::Request { id, method, .. })) => {
                if method == methods::AGENT_PERMISSION.name {
                    h.client_conn
                        .respond(id, serde_json::json!({"allow": false}))
                        .await
                        .unwrap();
                    permission_received = true;
                    break;
                }
            }
            Ok(Some(Incoming::Notification { .. })) => continue,
            _ => break,
        }
    }
    assert!(permission_received, "should receive permission request");

    let events = h.collect_events().await;
    assert!(
        has_stream(&events, "adjusted"),
        "LLM should respond after denial"
    );
}

/// Path 6: Two independent sessions via HubFrontend — verifies session lifecycle.
#[tokio::test]
async fn hub_two_independent_sessions() {
    // Session 1
    let mut h1 = build_hub_harness(scenarios::simple_text("session1 answer")).await;
    h1.send_message("session1 question").await;
    let ev1 = h1.collect_events().await;
    assert!(has_stream(&ev1, "session1 answer"));

    // Drop first harness (closes channels, agent loop exits)
    drop(h1);

    // Session 2 — independent, verifies no state leakage
    let mut h2 = build_hub_harness(scenarios::simple_text("session2 answer")).await;
    h2.send_message("session2 question").await;
    let ev2 = h2.collect_events().await;
    assert!(has_stream(&ev2, "session2 answer"));
}

/// Path 7: Five-turn extended conversation through HubFrontend.
#[tokio::test]
async fn hub_conversation_continues_after_many_turns() {
    let responses: Vec<&str> = vec!["t1-ok", "t2-ok", "t3-ok", "t4-ok", "t5-ok"];
    let mut h = build_hub_harness(scenarios::n_turn(&responses)).await;

    for (i, expected) in responses.iter().enumerate() {
        h.send_message(&format!("turn-{i}")).await;
        let events = h.collect_events().await;
        assert!(
            has_stream(&events, expected),
            "turn {i}: expected '{expected}'"
        );
    }
}
