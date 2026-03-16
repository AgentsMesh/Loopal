//! Edge cases and regression tests for tool call/result handling.

use loopagent_tui::app::{App, DisplayMessage, DisplayToolCall};
use loopagent_types::event::AgentEvent;
use tokio::sync::mpsc;

fn make_app() -> App {
    let (tx, _rx) = mpsc::channel::<AgentEvent>(16);
    App::new("test-model".to_string(), "act".to_string(), tx)
}

#[test]
fn test_handle_tool_result_no_matching_pending() {
    let mut app = make_app();
    app.handle_agent_event(AgentEvent::ToolResult {
        id: "orphan".to_string(),
        name: "bash".to_string(),
        result: "orphan result".to_string(),
        is_error: false,
    });
    // Should not crash
}

#[test]
fn test_tool_call_without_prior_assistant_message_creates_one() {
    let mut app = make_app();
    app.messages.push(DisplayMessage {
        role: "user".to_string(),
        content: "do something".to_string(),
        tool_calls: Vec::new(),
    });
    app.handle_agent_event(AgentEvent::ToolCall {
        id: "tc-new".to_string(),
        name: "Read".to_string(),
        input: serde_json::json!({}),
    });

    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[1].role, "assistant");
    assert_eq!(app.messages[1].tool_calls.len(), 1);
}

#[test]
fn test_tool_result_error_updates_matching_tool() {
    let mut app = make_app();
    app.handle_agent_event(AgentEvent::ToolCall {
        id: "tc-err".to_string(),
        name: "Write".to_string(),
        input: serde_json::json!({}),
    });
    app.handle_agent_event(AgentEvent::ToolResult {
        id: "tc-err".to_string(),
        name: "Write".to_string(),
        result: "failed!".to_string(),
        is_error: true,
    });

    assert_eq!(app.messages[0].tool_calls[0].status, "error");
    assert_eq!(app.messages[0].tool_calls[0].summary, "failed!");
}

#[test]
fn test_tool_result_not_found_when_different_name() {
    let mut app = make_app();
    app.handle_agent_event(AgentEvent::ToolCall {
        id: "tc-1".to_string(),
        name: "bash".to_string(),
        input: serde_json::json!({}),
    });
    app.handle_agent_event(AgentEvent::ToolResult {
        id: "tc-1".to_string(),
        name: "Read".to_string(),
        result: "done".to_string(),
        is_error: false,
    });

    assert_eq!(app.messages[0].tool_calls[0].status, "pending");
}

#[test]
fn test_tool_result_with_multibyte_utf8_no_panic() {
    let mut app = make_app();
    app.messages.push(DisplayMessage {
        role: "assistant".to_string(),
        content: String::new(),
        tool_calls: vec![DisplayToolCall {
            name: "Read".to_string(),
            status: "pending".to_string(),
            summary: "Read(...)".to_string(),
        }],
    });

    let chinese_text = "# Coding Agent 架构综合分析与最终建议报告\n\n> 分析日期: 2026-03-13\n> 输入来源: 5 份架构分析报告";
    app.handle_agent_event(AgentEvent::ToolResult {
        id: "tc-1".to_string(),
        name: "Read".to_string(),
        result: chinese_text.to_string(),
        is_error: false,
    });

    let tc = &app.messages[0].tool_calls[0];
    assert_eq!(tc.status, "success");
    assert!(tc.summary.ends_with("...") || tc.summary.len() <= 80);
}

#[test]
fn test_tool_call_with_multibyte_json_no_panic() {
    let mut app = make_app();
    app.handle_agent_event(AgentEvent::ToolCall {
        id: "tc-2".to_string(),
        name: "Read".to_string(),
        input: serde_json::json!({"file_path": "/tmp/中文路径/测试文件很长的名字用来超过截断限制.rs"}),
    });

    assert_eq!(app.messages.len(), 1);
    let tc = &app.messages[0].tool_calls[0];
    assert!(tc.summary.contains("Read"));
}
