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

/// Handle token usage update event.
pub fn handle_token_usage(
    state: &mut SessionState,
    input: u32,
    output: u32,
    context_window: u32,
    cache_creation: u32,
    cache_read: u32,
) {
    state.input_tokens = input;
    state.output_tokens = output;
    state.context_window = context_window;
    state.cache_creation_tokens = cache_creation;
    state.cache_read_tokens = cache_read;
    if input == 0 && output == 0 {
        state.thinking_tokens = 0;
    }
}

/// Handle auto-continuation event.
pub fn handle_auto_continuation(state: &mut SessionState, cont: u32, max: u32) {
    push_system_msg(state, &format!("Output truncated (max_tokens). Auto-continuing ({cont}/{max})"));
}

/// Handle context compaction event.
pub fn handle_compaction(
    state: &mut SessionState,
    kept: usize,
    removed: usize,
    tokens_before: u32,
    tokens_after: u32,
    strategy: &str,
) {
    let freed = tokens_before.saturating_sub(tokens_after);
    let pct = if tokens_before > 0 {
        freed * 100 / tokens_before
    } else {
        0
    };
    push_system_msg(
        state,
        &format!(
            "Context compacted ({strategy}): {removed} messages removed, \
             {kept} kept. {tokens_before}→{tokens_after} tokens ({pct}% freed).",
        ),
    );
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
