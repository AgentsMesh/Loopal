use crate::agent_conversation::AgentConversation;
use crate::types::SessionMessage;

/// Handle ThinkingComplete: flush thinking buffer and create full-content message.
///
/// Content format: `"{token_count}\n{full_thinking_text}"`.
/// The TUI renderer parses this to show both the token count header
/// and the full thinking body.
pub fn handle_thinking_complete(conv: &mut AgentConversation, token_count: u32) {
    conv.thinking_active = false;
    conv.thinking_tokens += token_count;
    if !conv.streaming_thinking.is_empty() {
        let thinking = std::mem::take(&mut conv.streaming_thinking);
        let content = format_thinking_content(&thinking, token_count);
        conv.messages.push(SessionMessage {
            role: "thinking".to_string(),
            content,
            tool_calls: Vec::new(),
            image_count: 0,
            skill_info: None,
        });
    }
}

/// Encode thinking content for storage: `"{token_count}\n{full_text}"`.
pub fn format_thinking_content(thinking: &str, token_count: u32) -> String {
    format!("{token_count}\n{thinking}")
}

/// Parse stored thinking content back into (token_count, text).
pub fn parse_thinking_content(content: &str) -> (u32, &str) {
    match content.split_once('\n') {
        Some((first, rest)) => {
            let count = first.parse::<u32>().unwrap_or(0);
            (count, rest)
        }
        None => (0, content),
    }
}

/// Format token count for display (e.g. "1.5k" or "500").
pub fn format_token_display(token_count: u32) -> String {
    if token_count >= 1000 {
        format!("{:.1}k", token_count as f64 / 1000.0)
    } else {
        format!("{token_count}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_thinking_content() {
        let content = format_thinking_content("Hello world\nsecond line", 1500);
        let (count, text) = parse_thinking_content(&content);
        assert_eq!(count, 1500);
        assert_eq!(text, "Hello world\nsecond line");
    }

    #[test]
    fn parse_legacy_format_fallback() {
        // Content without parseable token count falls back gracefully
        let (count, text) = parse_thinking_content("just plain text");
        assert_eq!(count, 0);
        assert_eq!(text, "just plain text");
    }

    #[test]
    fn format_token_display_large() {
        assert_eq!(format_token_display(1500), "1.5k");
    }

    #[test]
    fn format_token_display_small() {
        assert_eq!(format_token_display(500), "500");
    }
}
