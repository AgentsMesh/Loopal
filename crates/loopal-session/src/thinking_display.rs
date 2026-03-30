use crate::state::SessionState;
use crate::types::DisplayMessage;

/// Handle ThinkingComplete: flush thinking buffer and create summary message.
pub fn handle_thinking_complete(state: &mut SessionState, token_count: u32) {
    state.thinking_active = false;
    state.thinking_tokens += token_count;
    if !state.streaming_thinking.is_empty() {
        let thinking = std::mem::take(&mut state.streaming_thinking);
        let summary = format_thinking_summary(&thinking, token_count);
        state.messages.push(DisplayMessage {
            role: "thinking".to_string(),
            content: summary,
            tool_calls: Vec::new(),
            image_count: 0,
            skill_info: None,
        });
    }
}

/// Format a thinking summary for display.
pub fn format_thinking_summary(thinking: &str, token_count: u32) -> String {
    let token_display = if token_count >= 1000 {
        format!("{:.1}k", token_count as f64 / 1000.0)
    } else {
        format!("{token_count}")
    };
    // Take first line as preview
    let preview = thinking
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(80)
        .collect::<String>();
    format!("[{token_display} tokens] {preview}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_large_token_count() {
        let result = format_thinking_summary("Hello world\nsecond line", 1500);
        assert!(result.contains("1.5k tokens"));
        assert!(result.contains("Hello world"));
    }

    #[test]
    fn format_small_token_count() {
        let result = format_thinking_summary("Short", 500);
        assert!(result.contains("500 tokens"));
    }
}
