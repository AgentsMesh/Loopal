//! Display state helpers for conversation rendering.

use crate::agent_conversation::AgentConversation;
use crate::types::SessionMessage;

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

/// Push a system-role display message into the agent conversation.
pub fn push_system_msg(conv: &mut AgentConversation, content: &str) {
    conv.messages.push(SessionMessage {
        role: "system".into(),
        content: content.into(),
        tool_calls: Vec::new(),
        image_count: 0,
        skill_info: None,
    });
}

/// Handle token usage update event.
pub fn handle_token_usage(
    conv: &mut AgentConversation,
    input: u32,
    output: u32,
    context_window: u32,
    cache_creation: u32,
    cache_read: u32,
) {
    conv.input_tokens = input;
    conv.output_tokens = output;
    conv.context_window = context_window;
    conv.cache_creation_tokens = cache_creation;
    conv.cache_read_tokens = cache_read;
    if input == 0 && output == 0 {
        conv.thinking_tokens = 0;
    }
}

/// Handle auto-continuation event.
pub fn handle_auto_continuation(conv: &mut AgentConversation, cont: u32, max: u32) {
    push_system_msg(
        conv,
        &format!("Output truncated (max_tokens). Auto-continuing ({cont}/{max})"),
    );
}

/// Handle context compaction event.
pub fn handle_compaction(
    conv: &mut AgentConversation,
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
        conv,
        &format!(
            "Context compacted ({strategy}): {removed} messages removed, \
             {kept} kept. {tokens_before}→{tokens_after} tokens ({pct}% freed).",
        ),
    );
}
