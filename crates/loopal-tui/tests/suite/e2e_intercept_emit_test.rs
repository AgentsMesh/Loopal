//! EnterPlanMode emit ToolResult invariant (request_idle + AskUser covered elsewhere).

use std::time::Duration;

use loopal_protocol::AgentEventPayload;
use loopal_provider_api::Message;
use loopal_runtime::{AgentMode, LifecycleMode};
use loopal_test_support::assertions::find_tool_result_full;
use loopal_test_support::events::collect_until_idle;
use loopal_test_support::{HarnessBuilder, chunks};

const TIMEOUT: Duration = Duration::from_secs(5);

async fn run_intercept_emit_scenario(
    mode: AgentMode,
    tool_input_id: &str,
) -> Vec<AgentEventPayload> {
    let calls = vec![chunks::tool_turn(
        tool_input_id,
        "EnterPlanMode",
        serde_json::json!({}),
    )];
    let harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![Message::user("trigger plan intercept")])
        .mode(mode)
        .lifecycle(LifecycleMode::Ephemeral)
        .build()
        .await;
    let mut rx = harness.event_rx;
    let mut runner = harness.runner;
    let run_handle = tokio::spawn(async move {
        let _ = runner.run().await;
    });
    let events = collect_until_idle(&mut rx, TIMEOUT, |_| {}).await;
    let _ = run_handle.await;
    events
}

#[tokio::test]
async fn enter_plan_in_ephemeral_emits_tool_result_error() {
    let events = run_intercept_emit_scenario(AgentMode::Act, "tc-enter-plan").await;
    let (is_error, result) =
        find_tool_result_full(&events, "EnterPlanMode").expect("EnterPlanMode ToolResult required");
    assert!(is_error, "agent-context 拒绝时 is_error 应为 true");
    assert!(
        result.contains("cannot be used in agent contexts"),
        "got: {result}"
    );
}

#[tokio::test]
async fn enter_plan_when_already_in_plan_mode_emits_tool_result_error() {
    let events = run_intercept_emit_scenario(AgentMode::Plan, "tc-enter-plan-2").await;
    let (is_error, result) =
        find_tool_result_full(&events, "EnterPlanMode").expect("EnterPlanMode ToolResult required");
    assert!(is_error);
    assert!(result.contains("Already in plan mode"), "got: {result}");
}
