//! Small helper functions used by the session controller.

use crate::state::SessionState;
use crate::thinking_display::format_thinking_summary;
use crate::types::DisplayMessage;

/// Extract a human-readable label from a ThinkingConfig JSON string.
pub fn thinking_label_from_json(json: &str) -> String {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        return "unknown".into();
    };
    match v.get("type").and_then(|t| t.as_str()) {
        Some("auto") => "auto".into(),
        Some("disabled") => "disabled".into(),
        Some("effort") => v
            .get("level")
            .and_then(|l| l.as_str())
            .unwrap_or("medium")
            .into(),
        Some("budget") => {
            format!(
                "budget({})",
                v.get("tokens").and_then(|t| t.as_u64()).unwrap_or(0)
            )
        }
        _ => "unknown".into(),
    }
}

/// Push a system-role display message into the session state.
pub fn push_system_msg(state: &mut SessionState, content: &str) {
    state.messages.push(DisplayMessage {
        role: "system".into(),
        content: content.into(),
        tool_calls: Vec::new(),
        image_count: 0,
    });
}

/// Flush buffered streaming text into a DisplayMessage.
pub fn flush_streaming(state: &mut SessionState) {
    if !state.streaming_thinking.is_empty() {
        let thinking = std::mem::take(&mut state.streaming_thinking);
        let token_est = thinking.len() as u32 / 4;
        let summary = format_thinking_summary(&thinking, token_est);
        state.messages.push(DisplayMessage {
            role: "thinking".to_string(),
            content: summary,
            tool_calls: Vec::new(),
            image_count: 0,
        });
        state.thinking_active = false;
    }

    if !state.streaming_text.is_empty() {
        let text = std::mem::take(&mut state.streaming_text);
        if let Some(last) = state.messages.last_mut()
            && last.role == "assistant"
            && last.tool_calls.is_empty()
        {
            last.content.push_str(&text);
            return;
        }
        state.messages.push(DisplayMessage {
            role: "assistant".to_string(),
            content: text,
            tool_calls: Vec::new(),
            image_count: 0,
        });
    }
}
