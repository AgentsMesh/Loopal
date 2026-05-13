use loopal_tui::views::progress::message_to_lines;
use loopal_view_state::SessionMessage;

use crate::message_lines_fixture::{all_text, done_failure, done_success, msg, pending_call};

#[test]
fn test_thinking_shows_full_content() {
    let content = format!("2000\n{}", "x".repeat(200));
    let m = msg("thinking", &content);
    let lines = message_to_lines(&m, 80);
    assert!(
        lines.len() > 3,
        "thinking should show full content, got {}",
        lines.len()
    );
    let text = all_text(&lines);
    assert!(text.contains("Thinking"));
    assert!(text.contains("2.0k tokens"));
    assert!(text.contains("xxxx"));
}

#[test]
fn test_thinking_empty_shows_header_only() {
    let m = msg("thinking", "");
    let lines = message_to_lines(&m, 80);
    let text = all_text(&lines);
    assert!(text.contains("Thinking"));
}

#[test]
fn test_thinking_small_token_count() {
    let content = "500\nShort thinking content";
    let m = msg("thinking", content);
    let lines = message_to_lines(&m, 80);
    let text = all_text(&lines);
    assert!(text.contains("500 tokens"));
    assert!(text.contains("Short thinking content"));
}

#[test]
fn test_error_role_has_prefix() {
    let m = msg("error", "something went wrong");
    let lines = message_to_lines(&m, 80);
    let text = all_text(&lines);
    assert!(text.contains("Error: "));
    assert!(text.contains("something went wrong"));
}

#[test]
fn test_system_role_has_prefix() {
    let m = msg("system", "max turns reached");
    let lines = message_to_lines(&m, 80);
    let text = all_text(&lines);
    assert!(text.contains("System: "));
}

#[test]
fn test_tool_call_single_line_summary() {
    let mut tc = pending_call("Read", "Read(src/main.rs)");
    tc.state = done_success("fn main() {}");
    let m = SessionMessage {
        role: "assistant".to_string(),
        content: String::new(),
        tool_calls: vec![tc],
        image_count: 0,
        skill_info: None,
        inbox: None,
        message_id: None,
        ui_local: false,
    };
    let lines = message_to_lines(&m, 80);
    let text = all_text(&lines);
    assert!(text.contains("●"));
    assert!(text.contains("Read"));
}

#[test]
fn test_tool_call_error_shows_cross() {
    let mut tc = pending_call("Bash", "Bash(npm test)");
    tc.state = done_failure("ENOENT: command not found");
    let m = SessionMessage {
        role: "assistant".to_string(),
        content: String::new(),
        tool_calls: vec![tc],
        image_count: 0,
        skill_info: None,
        inbox: None,
        message_id: None,
        ui_local: false,
    };
    let lines = message_to_lines(&m, 80);
    let text = all_text(&lines);
    assert!(text.contains("●"));
}

#[test]
fn test_tool_call_pending_shows_spinner() {
    let tc = pending_call("Edit", "Edit(src/lib.rs)");
    let m = SessionMessage {
        role: "assistant".to_string(),
        content: String::new(),
        tool_calls: vec![tc],
        image_count: 0,
        skill_info: None,
        inbox: None,
        message_id: None,
        ui_local: false,
    };
    let lines = message_to_lines(&m, 80);
    let text = all_text(&lines);
    assert!(
        text.contains("⠋") || text.contains("●") || text.contains("⠙") || text.contains("⠹"),
        "pending should have spinner: {text}"
    );
}

#[test]
fn test_assistant_with_content_and_tools() {
    let mut tc = pending_call("Edit", "Edit(src/lib.rs:42)");
    tc.state = done_success("applied");
    let m = SessionMessage {
        role: "assistant".to_string(),
        content: "Let me fix this.".to_string(),
        tool_calls: vec![tc],
        image_count: 0,
        skill_info: None,
        inbox: None,
        message_id: None,
        ui_local: false,
    };
    let lines = message_to_lines(&m, 80);
    let text = all_text(&lines);
    assert!(text.contains("Let me fix this"));
    assert!(text.contains("●"));
    assert!(text.contains("Edit"));
}
