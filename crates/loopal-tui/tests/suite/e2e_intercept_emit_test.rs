//! intercept handler 的 ToolResult emit invariant 守门。
//!
//! 这是 plan Part B1 提到的"测试模板"：每个 intercept handler 都必须 emit
//! AgentEventPayload::ToolResult，让 view-state 把 invocation 标 Done 而不是被
//! turn-end reconcile 强标 Stale。
//!
//! 修复前 4 个 handler 里有 3 个漏 emit（request_idle / EnterPlanMode / ExitPlanMode）。
//! request_idle 的覆盖在 `e2e_request_idle_terminates_turn_test.rs`；
//! AskUser 的覆盖隐含在 `e2e_question_test.rs`；
//! 本文件补 EnterPlanMode 的最简单路径（agent context 拒绝 + already-in-plan-mode）。

use std::time::Duration;

use loopal_message::Message;
use loopal_protocol::AgentEventPayload;
use loopal_runtime::{AgentMode, LifecycleMode};
use loopal_test_support::{HarnessBuilder, chunks};

#[tokio::test]
async fn enter_plan_in_ephemeral_emits_tool_result_error() {
    // LifecycleMode::Ephemeral 路径 → handle_enter_plan 早期 return
    // "EnterPlanMode cannot be used in agent contexts"
    // 修复前这条路径不 emit ToolResult，view-state 中 invocation 会被
    // 卡到 turn-end 才标 Stale。
    let calls = vec![chunks::tool_turn(
        "tc-enter-plan",
        "EnterPlanMode",
        serde_json::json!({}),
    )];

    let harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![Message::user("try plan mode")])
        .lifecycle(LifecycleMode::Ephemeral)
        .build()
        .await;

    let event_rx = harness.event_rx;
    let mut runner = harness.runner;

    // Ephemeral lifecycle → runner.run() 跑完 turn 后正常 return
    let run_handle = tokio::spawn(async move {
        let _ = runner.run().await;
    });

    let mut saw_tool_result = false;
    let mut rx = event_rx;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let event = match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Some(e)) => e,
            Ok(None) => break,
            Err(_) => panic!("timeout waiting for ToolResult"),
        };
        if let AgentEventPayload::ToolResult {
            name,
            is_error,
            result,
            ..
        } = &event.payload
            && name == "EnterPlanMode"
        {
            assert!(*is_error, "agent-context 拒绝时 is_error 应为 true");
            assert!(
                result.contains("cannot be used in agent contexts"),
                "result 应包含拒绝原因；got: {result}"
            );
            saw_tool_result = true;
            break;
        }
    }

    let _ = run_handle.await;

    assert!(
        saw_tool_result,
        "EnterPlanMode 在 Ephemeral 路径上必须 emit ToolResult（修复前漏 emit）"
    );
}

#[tokio::test]
async fn enter_plan_when_already_in_plan_mode_emits_tool_result_error() {
    // mode=Plan → handle_enter_plan 早期 return "Already in plan mode."
    // 同样需要 emit ToolResult。
    let calls = vec![chunks::tool_turn(
        "tc-enter-plan-2",
        "EnterPlanMode",
        serde_json::json!({}),
    )];

    let harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![Message::user("try entering plan when already")])
        .mode(AgentMode::Plan)
        .lifecycle(LifecycleMode::Ephemeral)
        .build()
        .await;

    let mut rx = harness.event_rx;
    let mut runner = harness.runner;

    let run_handle = tokio::spawn(async move {
        let _ = runner.run().await;
    });

    let mut saw_tool_result = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let event = match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Some(e)) => e,
            Ok(None) => break,
            Err(_) => panic!("timeout waiting for ToolResult"),
        };
        if let AgentEventPayload::ToolResult {
            name,
            is_error,
            result,
            ..
        } = &event.payload
            && name == "EnterPlanMode"
        {
            assert!(*is_error);
            assert!(
                result.contains("Already in plan mode"),
                "got: {result}"
            );
            saw_tool_result = true;
            break;
        }
    }

    let _ = run_handle.await;
    assert!(saw_tool_result);
}
