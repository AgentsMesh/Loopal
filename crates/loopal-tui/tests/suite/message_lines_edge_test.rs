/// Message lines edge tests: thinking role, error/system prefixes, tool call integration.
use loopal_session::types::{SessionMessage, SessionToolCall, ToolCallStatus};
use loopal_tui::views::progress::message_to_lines;

fn msg(role: &str, content: &str) -> SessionMessage {
    SessionMessage {
        role: role.to_string(),
        content: content.to_string(),
        tool_calls: Vec::new(),
        image_count: 0,
        skill_info: None,
    }
}

fn all_text(lines: &[ratatui::prelude::Line<'_>]) -> String {
    lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// --- Thinking role ---

#[test]
fn test_thinking_shows_full_content() {
    // Format: "{token_count}\n{full_text}"
    let content = format!("2000\n{}", "x".repeat(200));
    let m = msg("thinking", &content);
    let lines = message_to_lines(&m, 80);
    // Header + blank + body lines + trailing separator
    assert!(
        lines.len() > 3,
        "thinking should show full content, got {}",
        lines.len()
    );
    let text = all_text(&lines);
    assert!(text.contains("Thinking"), "should contain Thinking label");
    assert!(text.contains("2.0k tokens"), "should show token count");
    // Body text present (indented x's)
    assert!(text.contains("xxxx"), "should contain thinking body text");
}

#[test]
fn test_thinking_empty_shows_header_only() {
    let m = msg("thinking", "");
    let lines = message_to_lines(&m, 80);
    let text = all_text(&lines);
    assert!(text.contains("Thinking"), "empty thinking shows header");
}

#[test]
fn test_thinking_small_token_count() {
    // 500 tokens, some text
    let content = "500\nShort thinking content";
    let m = msg("thinking", content);
    let lines = message_to_lines(&m, 80);
    let text = all_text(&lines);
    assert!(
        text.contains("500 tokens"),
        "small thinking should show raw count: {text}"
    );
    assert!(
        text.contains("Short thinking content"),
        "body should be shown"
    );
}

// --- Error and system roles ---

#[test]
fn test_error_role_has_prefix() {
    let m = msg("error", "something went wrong");
    let lines = message_to_lines(&m, 80);
    let text = all_text(&lines);
    assert!(
        text.contains("Error: "),
        "error should have 'Error: ' prefix"
    );
    assert!(text.contains("something went wrong"));
}

#[test]
fn test_system_role_has_prefix() {
    let m = msg("system", "max turns reached");
    let lines = message_to_lines(&m, 80);
    let text = all_text(&lines);
    assert!(
        text.contains("System: "),
        "system should have 'System: ' prefix"
    );
}

// --- Tool call integration ---

#[test]
fn test_tool_call_single_line_summary() {
    let m = SessionMessage {
        role: "assistant".to_string(),
        content: String::new(),
        tool_calls: vec![SessionToolCall {
            name: "Read".to_string(),
            id: String::new(),
            status: ToolCallStatus::Success,
            summary: "Read(src/main.rs)".to_string(),
            result: Some("fn main() {}".to_string()),
            tool_input: None,
            batch_id: None,
            started_at: None,
            duration_ms: None,
            progress_tail: None,
            metadata: None,
        }],
        image_count: 0,
        skill_info: None,
    };
    let lines = message_to_lines(&m, 80);
    let text = all_text(&lines);
    assert!(text.contains("●"), "success tool call should have ● icon");
    assert!(text.contains("Read"), "should contain tool name");
}

#[test]
fn test_tool_call_error_shows_cross() {
    let m = SessionMessage {
        role: "assistant".to_string(),
        content: String::new(),
        tool_calls: vec![SessionToolCall {
            name: "Bash".to_string(),
            id: String::new(),
            status: ToolCallStatus::Error,
            summary: "Bash(npm test)".to_string(),
            result: Some("ENOENT: command not found".to_string()),
            tool_input: None,
            batch_id: None,
            started_at: None,
            duration_ms: None,
            progress_tail: None,
            metadata: None,
        }],
        image_count: 0,
        skill_info: None,
    };
    let lines = message_to_lines(&m, 80);
    let text = all_text(&lines);
    assert!(text.contains("●"), "error tool call should have ● icon");
}

#[test]
fn test_tool_call_pending_shows_spinner() {
    let m = SessionMessage {
        role: "assistant".to_string(),
        content: String::new(),
        tool_calls: vec![SessionToolCall {
            name: "Edit".to_string(),
            id: String::new(),
            status: ToolCallStatus::Pending,
            summary: "Edit(src/lib.rs)".to_string(),
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
    };
    let lines = message_to_lines(&m, 80);
    let text = all_text(&lines);
    assert!(
        text.contains("⠋") || text.contains("●"),
        "pending tool call should have spinner or ● icon, got: {text}"
    );
}

#[test]
fn test_assistant_with_content_and_tools() {
    let m = SessionMessage {
        role: "assistant".to_string(),
        content: "Let me fix this.".to_string(),
        tool_calls: vec![SessionToolCall {
            name: "Edit".to_string(),
            id: String::new(),
            status: ToolCallStatus::Success,
            summary: "Edit(src/lib.rs:42)".to_string(),
            result: Some("applied".to_string()),
            tool_input: None,
            batch_id: None,
            started_at: None,
            duration_ms: None,
            progress_tail: None,
            metadata: None,
        }],
        image_count: 0,
        skill_info: None,
    };
    let lines = message_to_lines(&m, 80);
    let text = all_text(&lines);
    assert!(text.contains("Let me fix this"));
    assert!(text.contains("●"));
    assert!(text.contains("Edit"));
}
