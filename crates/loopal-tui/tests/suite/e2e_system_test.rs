//! E2E system-level tests: event ordering, token accumulation, unicode, thinking.

use loopal_protocol::AgentEventPayload;
use loopal_test_support::{assertions, chunks, events};

use super::e2e_harness::build_tui_harness;

#[tokio::test]
async fn test_event_ordering() {
    let mut harness = build_tui_harness(vec![chunks::text_turn("ordered")], 80, 24).await;
    let evts = harness.collect_until_idle().await;

    // Find positions of key events
    let started = evts
        .iter()
        .position(|e| matches!(e, AgentEventPayload::Started));
    let stream = evts
        .iter()
        .position(|e| matches!(e, AgentEventPayload::Stream { .. }));
    let usage = evts
        .iter()
        .position(|e| matches!(e, AgentEventPayload::TokenUsage { .. }));
    let finished = evts
        .iter()
        .position(|e| matches!(e, AgentEventPayload::Finished));

    assert!(started.is_some(), "should have Started");
    assert!(stream.is_some(), "should have Stream");
    assert!(finished.is_some(), "should have Finished");

    let s = started.unwrap();
    let st = stream.unwrap();
    let f = finished.unwrap();
    assert!(s < st, "Started({s}) should come before Stream({st})");
    assert!(st < f, "Stream({st}) should come before Finished({f})");

    if let Some(u) = usage {
        assert!(st < u, "Stream({st}) should come before TokenUsage({u})");
    }
}

#[tokio::test]
async fn test_token_accumulation() {
    // Two tool turns + text turn → multiple usage events
    let calls = vec![
        chunks::tool_turn("tc-1", "Ls", serde_json::json!({"path": "/tmp"})),
        chunks::tool_turn("tc-2", "Ls", serde_json::json!({"path": "/tmp"})),
        chunks::text_turn("done"),
    ];
    let mut harness = build_tui_harness(calls, 80, 24).await;
    let evts = harness.collect_until_idle().await;

    let total_input: u32 = evts
        .iter()
        .filter_map(|e| match e {
            AgentEventPayload::TokenUsage { input_tokens, .. } => Some(*input_tokens),
            _ => None,
        })
        .sum();
    let total_output: u32 = evts
        .iter()
        .filter_map(|e| match e {
            AgentEventPayload::TokenUsage { output_tokens, .. } => Some(*output_tokens),
            _ => None,
        })
        .sum();

    // Each tool_turn has usage(10,5) and text_turn has usage(5,3)
    assert!(total_input > 0, "should accumulate input tokens");
    assert!(total_output > 0, "should accumulate output tokens");
    assert_eq!(total_input, 25, "expected 10+10+5 input tokens");
    assert_eq!(total_output, 13, "expected 5+5+3 output tokens");
}

#[tokio::test]
async fn test_unicode_roundtrip() {
    let unicode_text = "你好世界🌍";
    let mut harness = build_tui_harness(vec![chunks::text_turn(unicode_text)], 100, 24).await;
    let evts = harness.collect_until_idle().await;

    let streamed = events::extract_texts(&evts);
    assert!(
        streamed.contains(unicode_text),
        "streamed text should contain unicode"
    );

    let rendered = harness.render_text();
    // CJK chars may be wide; just check the latin "world" emoji is present
    assertions::assert_buffer_contains(&rendered, "🌍");
}

#[tokio::test]
async fn test_thinking_stream() {
    let calls = vec![vec![
        chunks::thinking("Let me think about this..."),
        chunks::thinking_signature("sig-abc-123"),
        chunks::text("The answer is 42."),
        chunks::usage(10, 5),
        chunks::done(),
    ]];
    let mut harness = build_tui_harness(calls, 100, 30).await;
    let evts = harness.collect_until_idle().await;

    assertions::assert_has_thinking(&evts);
    assertions::assert_has_stream(&evts);
    assertions::assert_has_finished(&evts);

    let streamed = events::extract_texts(&evts);
    assert!(
        streamed.contains("42"),
        "should contain '42', got: {streamed}"
    );
}

#[tokio::test]
async fn test_finished_always_last_meaningful() {
    let mut harness = build_tui_harness(vec![chunks::text_turn("end")], 80, 24).await;
    let evts = harness.collect_until_idle().await;

    // Finished should be at or near the end (terminal event)
    let finished_pos = evts
        .iter()
        .rposition(|e| matches!(e, AgentEventPayload::Finished));
    assert!(finished_pos.is_some());
    let last_non_awaiting = evts
        .iter()
        .rposition(|e| !matches!(e, AgentEventPayload::AwaitingInput));
    assert_eq!(finished_pos, last_non_awaiting);
}
