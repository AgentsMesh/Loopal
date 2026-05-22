//! request_idle: emit ToolResult, terminate turn without next LLM call, signal turn-scoped.

use std::time::Duration;

use loopal_message::Message;
use loopal_protocol::{Envelope, MessageSource};
use loopal_test_support::events::{collect_until_idle, extract_tool_results};
use loopal_test_support::{HarnessBuilder, chunks};

const TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_IDLE_INPUT_JSON: &str = r#"{
    "max_idle_duration_secs": 600,
    "reason": "no more productive next action",
    "expected_wake_signal": "external envelope"
}"#;

#[tokio::test]
async fn request_idle_emits_tool_result_and_does_not_trigger_next_llm() {
    let calls = vec![chunks::tool_turn(
        "tc-idle-1",
        "request_idle",
        serde_json::from_str(REQUEST_IDLE_INPUT_JSON).unwrap(),
    )];

    let harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![Message::user("please go idle")])
        .build_spawned()
        .await;

    let recorded = harness.recorded_messages.clone();
    let mut event_rx = harness.event_rx;

    let events = collect_until_idle(&mut event_rx, TIMEOUT, |_| {}).await;
    let results = extract_tool_results(&events);

    assert!(
        results.iter().any(|(n, e)| n == "request_idle" && !*e),
        "expected request_idle ToolResult (is_error=false), got {results:?}"
    );
    assert_eq!(
        recorded.lock().unwrap().len(),
        1,
        "request_idle 后不应再发起 LLM call"
    );
}

#[tokio::test]
async fn request_idle_invalid_arg_also_emits_tool_result() {
    let calls = vec![chunks::tool_turn(
        "tc-idle-bad",
        "request_idle",
        serde_json::json!({"reason": "missing duration"}),
    )];

    let harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![Message::user("test bad input")])
        .build_spawned()
        .await;

    let mut event_rx = harness.event_rx;
    let events = collect_until_idle(&mut event_rx, TIMEOUT, |_| {}).await;
    let results = extract_tool_results(&events);

    assert!(
        results.iter().any(|(n, e)| n == "request_idle" && *e),
        "invalid arg 路径也必须 emit ToolResult is_error=true, got {results:?}"
    );
}

#[tokio::test]
async fn signal_does_not_leak_to_next_turn() {
    // turn 1 set 的信号必须在 turn 1 边界 reset, 不可污染 turn 2。
    let calls = vec![
        chunks::tool_turn(
            "tc-turn1-idle",
            "request_idle",
            serde_json::from_str(REQUEST_IDLE_INPUT_JSON).unwrap(),
        ),
        chunks::text_turn("ok, working on it"),
    ];

    let harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![Message::user("turn 1: please go idle")])
        .build_spawned()
        .await;

    let recorded = harness.recorded_messages.clone();
    let mailbox_tx = harness.mailbox_tx.clone();
    let mut event_rx = harness.event_rx;

    collect_until_idle(&mut event_rx, TIMEOUT, |_| {}).await;
    assert_eq!(
        recorded.lock().unwrap().len(),
        1,
        "turn 1 (request_idle) 应只调一次 LLM"
    );

    let env = Envelope::new(MessageSource::Human, "main", "turn 2: continue please");
    mailbox_tx
        .send(env)
        .await
        .expect("failed to send envelope for turn 2");

    collect_until_idle(&mut event_rx, TIMEOUT, |_| {}).await;
    assert_eq!(
        recorded.lock().unwrap().len(),
        2,
        "turn 2 必须调用 LLM (signal 不应跨 turn 泄漏)"
    );
}
