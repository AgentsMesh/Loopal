use loopal_protocol::MessageSource;

use super::agent_conversation::AgentConversation;
use super::types::{InboxOrigin, SessionMessage};

pub fn push_system_msg(conv: &mut AgentConversation, content: &str) {
    conv.messages.push(SessionMessage {
        role: "system".into(),
        content: content.into(),
        ..Default::default()
    });
}

pub fn push_inbox_msg(
    conv: &mut AgentConversation,
    message_id: String,
    source: MessageSource,
    content: String,
    summary: Option<String>,
) {
    conv.messages.push(SessionMessage {
        role: "user".into(),
        content,
        inbox: Some(InboxOrigin {
            message_id,
            source,
            summary,
        }),
        ..Default::default()
    });
}

pub fn handle_token_usage(
    conv: &mut AgentConversation,
    input: u32,
    output: u32,
    context_window: u32,
    cache_creation: u32,
    cache_read: u32,
) {
    if input > 0 || cache_creation > 0 || cache_read > 0 {
        conv.input_tokens = input;
        conv.output_tokens = output;
        conv.cache_creation_tokens = cache_creation;
        conv.cache_read_tokens = cache_read;
    } else {
        conv.output_tokens = output;
    }
    conv.context_window = context_window;
    if input == 0 && output == 0 {
        conv.thinking_tokens = 0;
    }
}

pub fn handle_auto_continuation(conv: &mut AgentConversation, cont: u32, max: u32, reason: &str) {
    let label = match reason {
        "max_tokens_with_tools" => {
            "Output truncated during tool calls (max_tokens); incomplete tools discarded."
        }
        "pause_turn" => "Provider paused the turn.",
        "stream_truncated" => "Response stream ended unexpectedly.",
        _ => "Output truncated (max_tokens).",
    };
    push_system_msg(conv, &format!("{label} Auto-continuing ({cont}/{max})"));
}

pub fn handle_compaction(
    conv: &mut AgentConversation,
    kept: usize,
    summarized: usize,
    tokens_before: u32,
    tokens_after: u32,
    strategy: &str,
    files_rehydrated: usize,
) {
    let freed = tokens_before.saturating_sub(tokens_after);
    let pct = if tokens_before > 0 {
        freed * 100 / tokens_before
    } else {
        0
    };
    let rehydrate_suffix = match files_rehydrated {
        0 => String::new(),
        1 => ", 1 file rehydrated".to_string(),
        count => format!(", {count} files rehydrated"),
    };
    let before = kept + summarized;
    push_system_msg(
        conv,
        &format!(
            "Context compacted ({strategy}): {before}→{kept} messages \
             ({summarized} summarized), {tokens_before}→{tokens_after} tokens \
             ({pct}% freed){rehydrate_suffix}.",
        ),
    );
    // Self-correct ctx counter from the Compacted event alone, in case the
    // paired TokenUsage emit is dropped or reordered.
    conv.input_tokens = tokens_after;
    conv.output_tokens = 0;
    conv.cache_creation_tokens = 0;
    conv.cache_read_tokens = 0;
}
