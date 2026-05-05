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
    conv.input_tokens = input;
    conv.output_tokens = output;
    conv.context_window = context_window;
    conv.cache_creation_tokens = cache_creation;
    conv.cache_read_tokens = cache_read;
    if input == 0 && output == 0 {
        conv.thinking_tokens = 0;
    }
}

pub fn handle_auto_continuation(conv: &mut AgentConversation, cont: u32, max: u32) {
    push_system_msg(
        conv,
        &format!("Output truncated (max_tokens). Auto-continuing ({cont}/{max})"),
    );
}

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
