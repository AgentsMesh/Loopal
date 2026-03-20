//! Edge cases and regression tests for tool call/result handling.

use loopal_session::{DisplayMessage, DisplayToolCall, SessionController};
use loopal_tui::app::App;
use loopal_tui::command::builtin_entries;
use loopal_protocol::ControlCommand;
use loopal_protocol::{AgentEvent, AgentEventPayload, UserQuestionResponse};
use tokio::sync::mpsc;

fn make_app() -> App {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _) = mpsc::channel::<bool>(16);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        "test-model".to_string(),
        "act".to_string(),
        control_tx,
        perm_tx,
        question_tx,
    );
    App::new(session, builtin_entries(), std::env::temp_dir())
}

#[test]
fn test_handle_tool_result_no_matching_pending() {
    let app = make_app();
    app.session.handle_event(AgentEvent::root(AgentEventPayload::ToolResult {
        id: "orphan".to_string(),
        name: "bash".to_string(),
        result: "orphan result".to_string(),
        is_error: false,
    }));
    // Should not crash
}

#[test]
fn test_tool_call_without_prior_assistant_message_creates_one() {
    let app = make_app();
    {
        let mut state = app.session.lock();
        state.messages.push(DisplayMessage {
            role: "user".to_string(),
            content: "do something".to_string(),
            tool_calls: Vec::new(),
        });
    }
    app.session.handle_event(AgentEvent::root(AgentEventPayload::ToolCall {
        id: "tc-new".to_string(),
        name: "Read".to_string(),
        input: serde_json::json!({}),
    }));

    let state = app.session.lock();
    assert_eq!(state.messages.len(), 2);
    assert_eq!(state.messages[1].role, "assistant");
    assert_eq!(state.messages[1].tool_calls.len(), 1);
}

#[test]
fn test_tool_result_error_updates_matching_tool() {
    let app = make_app();
    app.session.handle_event(AgentEvent::root(AgentEventPayload::ToolCall {
        id: "tc-err".to_string(),
        name: "Write".to_string(),
        input: serde_json::json!({}),
    }));
    app.session.handle_event(AgentEvent::root(AgentEventPayload::ToolResult {
        id: "tc-err".to_string(),
        name: "Write".to_string(),
        result: "failed!".to_string(),
        is_error: true,
    }));

    let state = app.session.lock();
    assert_eq!(state.messages[0].tool_calls[0].status, "error");
    assert_eq!(state.messages[0].tool_calls[0].result, Some("failed!".into()));
    assert!(state.messages[0].tool_calls[0].summary.contains("Write"));
}

#[test]
fn test_tool_result_not_found_when_different_name() {
    let app = make_app();
    app.session.handle_event(AgentEvent::root(AgentEventPayload::ToolCall {
        id: "tc-1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({}),
    }));
    app.session.handle_event(AgentEvent::root(AgentEventPayload::ToolResult {
        id: "tc-1".to_string(),
        name: "Read".to_string(),
        result: "done".to_string(),
        is_error: false,
    }));

    assert_eq!(app.session.lock().messages[0].tool_calls[0].status, "pending");
}

#[test]
fn test_tool_result_with_multibyte_utf8_no_panic() {
    let app = make_app();
    {
        let mut state = app.session.lock();
        state.messages.push(DisplayMessage {
            role: "assistant".to_string(),
            content: String::new(),
            tool_calls: vec![DisplayToolCall {
                name: "Read".to_string(),
                status: "pending".to_string(),
                summary: "Read(...)".to_string(),
                result: None,
            }],
        });
    }

    let chinese_text = "# Coding Agent 架构综合分析与最终建议报告\n\n> 分析日期: 2026-03-13\n> 输入来源: 5 份架构分析报告";
    app.session.handle_event(AgentEvent::root(AgentEventPayload::ToolResult {
        id: "tc-1".to_string(),
        name: "Read".to_string(),
        result: chinese_text.to_string(),
        is_error: false,
    }));

    let state = app.session.lock();
    let tc = &state.messages[0].tool_calls[0];
    assert_eq!(tc.status, "success");
    assert!(tc.result.is_some());
    assert!(tc.summary.contains("Read"));
}

#[test]
fn test_tool_call_with_multibyte_json_no_panic() {
    let app = make_app();
    app.session.handle_event(AgentEvent::root(AgentEventPayload::ToolCall {
        id: "tc-2".to_string(),
        name: "Read".to_string(),
        input: serde_json::json!({"file_path": "/tmp/中文路径/测试文件很长的名字用来超过截断限制.rs"}),
    }));

    let state = app.session.lock();
    assert_eq!(state.messages.len(), 1);
    let tc = &state.messages[0].tool_calls[0];
    assert!(tc.summary.contains("Read"));
}
