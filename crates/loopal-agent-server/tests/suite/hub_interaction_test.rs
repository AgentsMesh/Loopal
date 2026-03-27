//! Full-stack HubFrontend interaction tests for common user paths.
//!
//! Each test wires HubFrontend + real agent loop via hub_harness,
//! covering the gap between unit-level HubFrontend tests and TUI e2e tests.

use loopal_protocol::AgentEventPayload;
use loopal_test_support::scenarios;

use crate::hub_harness::{build_hub_harness, has_stream};

/// Path 2: AwaitingInput event → next message delivered → agent responds.
#[tokio::test]
async fn hub_awaiting_input_then_message_delivered() {
    let mut h = build_hub_harness(scenarios::two_turn("first", "second")).await;

    h.send_message("hello").await;
    let ev1 = h.collect_events().await;
    assert!(has_stream(&ev1, "first"), "should stream first response");
    assert!(ev1.iter().any(|e| matches!(e, AgentEventPayload::AwaitingInput)));

    h.send_message("continue").await;
    let ev2 = h.collect_events().await;
    assert!(has_stream(&ev2, "second"), "should stream second response");
}

/// Path 4: Three-round multi-turn conversation through HubFrontend.
#[tokio::test]
async fn hub_multi_turn_three_rounds() {
    let mut h = build_hub_harness(scenarios::n_turn(&["r1", "r2", "r3"])).await;

    for (i, expected) in ["r1", "r2", "r3"].iter().enumerate() {
        h.send_message(&format!("msg-{i}")).await;
        let events = h.collect_events().await;
        assert!(has_stream(&events, expected), "round {i}: expected {expected}");
    }
}

/// Path 1: Interrupt during slow tool execution → agent resumes with next message.
#[tokio::test]
async fn hub_interrupt_during_tool_then_resume() {
    // Call 1: Bash sleep (slow tool — will be interrupted)
    // Call 2: text response to the resumed message (after interrupt + new input)
    let calls = vec![
        loopal_test_support::chunks::tool_turn(
            "tc-1",
            "Bash",
            serde_json::json!({"command": "sleep 30", "timeout": 60000}),
        ),
        loopal_test_support::chunks::text_turn("resumed ok"),
    ];
    let mut h = build_hub_harness(calls).await;

    h.send_message("start slow task").await;

    // Wait a bit for the Bash tool to start executing
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Interrupt while Bash is sleeping
    h.interrupt();

    // Collect events from the interrupted turn
    let ev1 = h.collect_events().await;
    assert!(
        ev1.iter().any(|e| matches!(e, AgentEventPayload::Interrupted)),
        "should emit Interrupted event"
    );

    // Send new message — agent should resume (stale interrupt consumed)
    h.send_message("new task").await;
    let ev2 = h.collect_events().await;
    assert!(has_stream(&ev2, "resumed ok"), "should resume after interrupt");
}

/// Path 8: Three messages sent sequentially, all processed in order.
#[tokio::test]
async fn hub_sequential_messages_in_order() {
    let mut h = build_hub_harness(scenarios::n_turn(&["a1", "a2", "a3"])).await;

    // Send first message, wait for response
    h.send_message("m1").await;
    let ev1 = h.collect_events().await;
    assert!(has_stream(&ev1, "a1"));

    // Send second immediately after idle
    h.send_message("m2").await;
    let ev2 = h.collect_events().await;
    assert!(has_stream(&ev2, "a2"));

    // Send third
    h.send_message("m3").await;
    let ev3 = h.collect_events().await;
    assert!(has_stream(&ev3, "a3"));
}
