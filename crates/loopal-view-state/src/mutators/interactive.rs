use loopal_protocol::{MessageSource, ResolveSource};

use crate::SessionMessage;
use crate::conversation::{PendingPermission, conversation_display};
use crate::state::SessionViewState;

pub(super) fn tool_permission_request(
    state: &mut SessionViewState,
    id: &str,
    name: &str,
    input: &serde_json::Value,
) -> bool {
    let conv = &mut state.agent.conversation;
    conv.flush_streaming();
    conv.pending_permission = Some(PendingPermission {
        id: id.to_string(),
        name: name.to_string(),
        input: input.clone(),
    });
    true
}

/// Clear `pending_permission` if its id matches the resolved request.
/// Broadcast on race resolution so non-winning UIs hide the dialog.
pub(super) fn tool_permission_resolved(state: &mut SessionViewState, id: &str) -> bool {
    let pending = &mut state.agent.conversation.pending_permission;
    if pending.as_ref().is_some_and(|p| p.id == id) {
        *pending = None;
        true
    } else {
        false
    }
}

pub(super) fn user_message_queued(
    state: &mut SessionViewState,
    message_id: &str,
    content: &str,
    image_count: usize,
) -> bool {
    let already_present = state
        .agent
        .conversation
        .messages
        .iter()
        .any(|m| m.role == "user" && m.message_id.as_deref() == Some(message_id));
    if already_present {
        return false;
    }
    let mut text = content.to_string();
    if image_count > 0 {
        text.push_str(&format!(" [+{image_count} image(s)]"));
    }
    state.agent.conversation.messages.push(SessionMessage {
        role: "user".to_string(),
        content: text,
        image_count,
        message_id: Some(message_id.to_string()),
        ..Default::default()
    });
    true
}

pub(super) fn auto_continuation(state: &mut SessionViewState, cont: u32, max: u32) -> bool {
    conversation_display::handle_auto_continuation(&mut state.agent.conversation, cont, max);
    true
}

pub(super) fn compacted(
    state: &mut SessionViewState,
    kept: usize,
    removed: usize,
    tokens_before: u32,
    tokens_after: u32,
    strategy: &str,
) -> bool {
    conversation_display::handle_compaction(
        &mut state.agent.conversation,
        kept,
        removed,
        tokens_before,
        tokens_after,
        strategy,
    );
    true
}

pub(super) fn inbox_enqueued(
    state: &mut SessionViewState,
    message_id: &str,
    source: &MessageSource,
    content: &str,
    summary: Option<&str>,
) -> bool {
    if source.is_optimistically_rendered() {
        return false;
    }
    conversation_display::push_inbox_msg(
        &mut state.agent.conversation,
        message_id.to_string(),
        source.clone(),
        content.to_string(),
        summary.map(String::from),
    );
    true
}

pub(super) fn permission_decided(
    state: &mut SessionViewState,
    tool_name: &str,
    decision: &str,
    reason: &str,
    duration_ms: u64,
) -> bool {
    let label = match decision {
        "allow" => "permission allowed",
        "deny" => "permission denied",
        _ => "permission",
    };
    let suffix = if duration_ms > 0 {
        format!(" ({duration_ms}ms)")
    } else {
        String::new()
    };
    let line = if reason.is_empty() {
        format!("[{label}] {tool_name}{suffix}")
    } else {
        format!("[{label}] {tool_name}: {reason}{suffix}")
    };
    conversation_display::push_system_msg(&mut state.agent.conversation, &line);
    true
}

pub(super) fn question_decided(
    state: &mut SessionViewState,
    question_count: u32,
    reason: &str,
    duration_ms: u64,
    source: ResolveSource,
) -> bool {
    let suffix = if duration_ms > 0 {
        format!(" ({duration_ms}ms)")
    } else {
        String::new()
    };
    let label = format!("[ask-user] {}", source.as_str());
    let line = if reason.is_empty() {
        format!("{label}: {question_count} question(s){suffix}")
    } else {
        format!("{label}: {reason}{suffix}")
    };
    conversation_display::push_system_msg(&mut state.agent.conversation, &line);
    true
}
