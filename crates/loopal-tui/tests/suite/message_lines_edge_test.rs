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
fn test_thinking_collapsed_to_single_line() {
    let content = "x".repeat(8000); // ~2k tokens
    let m = msg("thinking", &content);
    let lines = message_to_lines(&m, 80);
    // 1 indicator line + 1 empty separator = 2
    assert_eq!(lines.len(), 2, "thinking should collapse to single line");
    let text = all_text(&lines);
    assert!(text.contains("Thinking"), "should contain Thinking label");
    assert!(text.contains("k tokens"), "should show token estimate");
}

#[test]
fn test_thinking_empty_shows_ellipsis() {
    let m = msg("thinking", "");
    let lines = message_to_lines(&m, 80);
    let text = all_text(&lines);
    assert!(
        text.contains("Thinking..."),
        "empty thinking shows ellipsis"
    );
}

#[test]
fn test_thinking_small_shows_raw_token_count() {
    // 400 bytes / 4 = 100 tokens — should show "100 tokens", not "0k tokens"
    let content = "x".repeat(400);
    let m = msg("thinking", &content);
    let lines = message_to_lines(&m, 80);
    let text = all_text(&lines);
    assert!(
        text.contains("100 tokens"),
        "small thinking should show raw count: {text}"
    );
    assert!(!text.contains("0k"), "should NOT show 0k: {text}");
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
    assert!(text.contains("●"), "pending tool call should have ● icon");
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
