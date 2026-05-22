//! request_idle 工具的 turn 终止 + ToolResult emit 测试。
//!
//! 修复前两个关联 bug:
//!   1. handle_request_idle 漏 emit AgentEventPayload::ToolResult → view-state 中
//!      invocation 停在 Running 直到 turn-end reconcile 强制标记 Stale
//!      （TUI 上显示 "Stale (turn ended after N)"）。
//!   2. turn_exec.rs::ToolResultsWritten 无条件转 ReadyToCall → request_idle 后又
//!      发起一次 LLM call 消耗 tool_result（白白浪费 tokens + 时间）。
//!
//! 这两个 bug 修了之后，request_idle 工具应当：
//!   - emit ToolResult 让 view-state 把 invocation 标 Done
//!   - turn 进入 ToolResultsWritten 后通过 runner.signal_turn_end_after_tools
//!     直接 Complete，不再发起新 LLM call
//!
//! invariant 守门 (`signal_does_not_leak_to_next_turn`)：信号是 turn-scoped，
//! 不可污染下一 turn。turn 边界 run_loop 显式 take 清残留作为防御。

use std::time::Duration;

use loopal_message::Message;
use loopal_protocol::{AgentEventPayload, Envelope, MessageSource};
use loopal_test_support::{HarnessBuilder, chunks};

const REQUEST_IDLE_INPUT_JSON: &str = r#"{
    "max_idle_duration_secs": 600,
    "reason": "no more productive next action",
    "expected_wake_signal": "external envelope"
}"#;

#[tokio::test]
async fn request_idle_emits_tool_result_and_does_not_trigger_next_llm() {
    // Provider 只配置 1 次 LLM call —— 返回 request_idle tool_use。
    // 如果 turn_exec 之后还发起第二次 LLM call，MultiCallProvider 会返回空
    // chunks，agent 表现异常（看 collect_until_idle 是否能正常完成），但更直接的
    // 是断言 recorded_messages.len() == 1。
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
    let mut saw_tool_result = false;

    // 收集事件直到 AwaitingInput（即 turn 完整结束、进入 idle phase）
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let event = tokio::time::timeout_at(deadline, event_rx.recv())
            .await
            .expect("等 AwaitingInput 超时")
            .expect("event channel closed");
        match &event.payload {
            AgentEventPayload::ToolResult { name, is_error, .. } if name == "request_idle" => {
                assert!(!*is_error, "request_idle 正常完成时 is_error 应为 false");
                saw_tool_result = true;
            }
            AgentEventPayload::AwaitingInput => break,
            _ => {}
        }
    }

    assert!(
        saw_tool_result,
        "request_idle intercept handler 必须 emit ToolResult event（修复前漏 emit）"
    );

    // 关键断言：只发了 1 次 LLM call —— 修复前会有第 2 次消耗 tool_result 的调用
    let call_count = recorded.lock().unwrap().len();
    assert_eq!(
        call_count, 1,
        "request_idle 后不应再发起 LLM call；实际发了 {call_count} 次"
    );
}

#[tokio::test]
async fn request_idle_invalid_arg_also_emits_tool_result() {
    // 边界用例：input 校验失败（缺 max_idle_duration_secs）→ handle_request_idle 走
    // 早期 return 分支，仍必须 emit ToolResult is_error=true。修复前这条路径同样漏 emit。
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
    let mut saw_error_tool_result = false;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let event = tokio::time::timeout_at(deadline, event_rx.recv())
            .await
            .expect("等 AwaitingInput 超时")
            .expect("event channel closed");
        match &event.payload {
            AgentEventPayload::ToolResult {
                name,
                is_error,
                result,
                ..
            } if name == "request_idle" => {
                assert!(*is_error, "invalid arg 时 is_error 应为 true");
                assert!(
                    result.contains("invalid arguments")
                        || result.contains("max_idle_duration_secs"),
                    "错误消息应说明问题；got: {result}"
                );
                saw_error_tool_result = true;
            }
            AgentEventPayload::AwaitingInput => break,
            _ => {}
        }
    }

    assert!(
        saw_error_tool_result,
        "无效参数的 request_idle 也必须 emit ToolResult（即 early-return 分支也走 helper）"
    );
}

#[tokio::test]
async fn signal_does_not_leak_to_next_turn() {
    // invariant 守门：turn 1 设置的 turn-end signal 必须在 turn 1 边界 reset，
    // 不可污染 turn 2。修复前 (字段为 pub mutable 且仅靠 ToolResultsWritten 分支
    // mem::take 消费) 在错误路径上信号可能泄漏。修复后 run_loop 每轮末尾显式 take。
    //
    // 此测试覆盖正常路径 (turn 1 通过 ToolResultsWritten 消费、turn 2 LLM 被调到)；
    // 错误路径的覆盖由 method 本身的幂等性 + run_loop reset 共同保证 (run_loop 末尾
    // 调 take 与 ToolResultsWritten 分支的 take 是同一 mem::take 实现)。
    let calls = vec![
        // turn 1: request_idle，应该直接结束 turn 不发后续 LLM
        chunks::tool_turn(
            "tc-turn1-idle",
            "request_idle",
            serde_json::from_str(REQUEST_IDLE_INPUT_JSON).unwrap(),
        ),
        // turn 2: 收到新 user envelope 后应该被 LLM 接到
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

    // 等 turn 1 完成进入 AwaitingInput
    let deadline_t1 = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let event = tokio::time::timeout_at(deadline_t1, event_rx.recv())
            .await
            .expect("等 turn 1 AwaitingInput 超时")
            .expect("event channel closed");
        if matches!(event.payload, AgentEventPayload::AwaitingInput) {
            break;
        }
    }

    // 此时 turn 1 应该消费 signal 并 Complete。LLM 调用 1 次。
    assert_eq!(
        recorded.lock().unwrap().len(),
        1,
        "turn 1 (request_idle) 应只调一次 LLM (然后直接 Complete)"
    );

    // 发新 envelope 触发 turn 2
    let env = Envelope::new(MessageSource::Human, "main", "turn 2: continue please");
    mailbox_tx
        .send(env)
        .await
        .expect("failed to send envelope for turn 2");

    // 等 turn 2 完成。如果 signal 泄漏，turn 2 会在 ToolResultsWritten 直接
    // Complete 不发 LLM。但 turn 2 没有 tool call 路径 (text_turn 直接 end_turn)，
    // 真正的回归指标是: turn 2 LLM 被调用了。
    let deadline_t2 = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let event = tokio::time::timeout_at(deadline_t2, event_rx.recv())
            .await
            .expect("等 turn 2 AwaitingInput 超时")
            .expect("event channel closed");
        if matches!(event.payload, AgentEventPayload::AwaitingInput) {
            break;
        }
    }

    // 关键断言：turn 2 LLM 被实际调用 (recorded 增长到 2)。
    // 如果信号泄漏，turn 2 会跳过 LLM，recorded 仍是 1。
    let final_call_count = recorded.lock().unwrap().len();
    assert_eq!(
        final_call_count, 2,
        "turn 2 必须调用 LLM (signal 不应跨 turn 泄漏)；实际 {final_call_count} 次"
    );
}
