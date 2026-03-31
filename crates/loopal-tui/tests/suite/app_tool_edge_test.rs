//! Edge cases and regression tests for tool call/result handling.

use loopal_protocol::ControlCommand;
use loopal_protocol::{AgentEvent, AgentEventPayload, UserQuestionResponse};
use loopal_session::{SessionController, SessionMessage, SessionToolCall, ToolCallStatus};
use loopal_tui::app::App;

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
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    App::new(session, std::env::temp_dir())
}

#[test]
fn test_handle_tool_result_no_matching_pending() {
    let app = make_app();
    app.session
        .handle_event(AgentEvent::root(AgentEventPayload::ToolResult {
            id: "orphan".to_string(),
            name: "bash".to_string(),
            result: "orphan result".to_string(),
            is_error: false,
            duration_ms: None,
            is_completion: false,
            metadata: None,
        }));
    // Should not crash
}

#[test]
fn test_tool_call_without_prior_assistant_message_creates_one() {
    let app = make_app();
    {
        let mut state = app.session.lock();
        state
            .active_conversation_mut()
            .messages
            .push(SessionMessage {
                role: "user".to_string(),
                content: "do something".to_string(),
                tool_calls: Vec::new(),
                image_count: 0,
                skill_info: None,
            });
    }
    app.session
        .handle_event(AgentEvent::root(AgentEventPayload::ToolCall {
            id: "tc-new".to_string(),
            name: "Read".to_string(),
            input: serde_json::json!({}),
        }));

    let state = app.session.lock();
    let conv = state.active_conversation();
    assert_eq!(conv.messages.len(), 2);
    assert_eq!(conv.messages[1].role, "assistant");
    assert_eq!(conv.messages[1].tool_calls.len(), 1);
}

#[test]
fn test_tool_result_error_updates_matching_tool() {
    let app = make_app();
    app.session
        .handle_event(AgentEvent::root(AgentEventPayload::ToolCall {
            id: "tc-err".to_string(),
            name: "Write".to_string(),
            input: serde_json::json!({}),
        }));
    app.session
        .handle_event(AgentEvent::root(AgentEventPayload::ToolResult {
            id: "tc-err".to_string(),
            name: "Write".to_string(),
            result: "failed!".to_string(),
            is_error: true,
            duration_ms: None,
            is_completion: false,
            metadata: None,
        }));

    let state = app.session.lock();
    let conv = state.active_conversation();
    assert_eq!(conv.messages[0].tool_calls[0].status, ToolCallStatus::Error);
    assert_eq!(
        conv.messages[0].tool_calls[0].result,
        Some("failed!".into())
    );
    assert!(conv.messages[0].tool_calls[0].summary.contains("Write"));
}

#[test]
fn test_tool_result_not_found_when_different_name() {
    let app = make_app();
    app.session
        .handle_event(AgentEvent::root(AgentEventPayload::ToolCall {
            id: "tc-1".to_string(),
            name: "bash".to_string(),
            input: serde_json::json!({}),
        }));
    app.session
        .handle_event(AgentEvent::root(AgentEventPayload::ToolResult {
            id: "tc-1".to_string(),
            name: "Read".to_string(),
            result: "done".to_string(),
            is_error: false,
            duration_ms: None,
            is_completion: false,
            metadata: None,
        }));

    // Now matches by id (not name), so the tool call IS updated
    assert_eq!(
        app.session.lock().active_conversation().messages[0].tool_calls[0].status,
        ToolCallStatus::Success
    );
}

#[test]
fn test_tool_result_with_multibyte_utf8_no_panic() {
    let app = make_app();
    {
        let mut state = app.session.lock();
        state
            .active_conversation_mut()
            .messages
            .push(SessionMessage {
                role: "assistant".to_string(),
                content: String::new(),
                tool_calls: vec![SessionToolCall {
                    name: "Read".to_string(),
                    id: "tc-1".to_string(),
                    status: ToolCallStatus::Pending,
                    summary: "Read(...)".to_string(),
                    result: None,
                    tool_input: None,
                    batch_id: None,
                    started_at: None,
                    duration_ms: None,
                    progress_tail: None,
                    metadata: None,
                }],
                image_count: 0,
                skill_info: None,
            });
    }

    let chinese_text = "# Coding Agent 架构综合分析与最终建议报告\n\n> 分析日期: 2026-03-13\n> 输入来源: 5 份架构分析报告";
    app.session
        .handle_event(AgentEvent::root(AgentEventPayload::ToolResult {
            id: "tc-1".to_string(),
            name: "Read".to_string(),
            result: chinese_text.to_string(),
            is_error: false,
            duration_ms: None,
            is_completion: false,
            metadata: None,
        }));

    let state = app.session.lock();
    let conv = state.active_conversation();
    let tc = &conv.messages[0].tool_calls[0];
    assert_eq!(tc.status, ToolCallStatus::Success);
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
    let conv = state.active_conversation();
    assert_eq!(conv.messages.len(), 1);
    let tc = &conv.messages[0].tool_calls[0];
    assert!(tc.summary.contains("Read"));
}
