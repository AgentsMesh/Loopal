use loopal_protocol::MessageSource;

use crate::SessionMessage;
use crate::conversation::{PendingPermission, PendingPlanApproval, conversation_display};
use crate::state::SessionViewState;

use super::MutationEffect;

pub(super) fn tool_permission_request(
    state: &mut SessionViewState,
    id: &str,
    name: &str,
    input: &serde_json::Value,
) -> MutationEffect {
    let conv = &mut state.agent.conversation;
    conv.flush_streaming();
    conv.pending_permission = Some(PendingPermission {
        id: id.to_string(),
        name: name.to_string(),
        input: input.clone(),
        cursor: Default::default(),
    });
    MutationEffect::Mutated
}

/// Clear `pending_permission` if its id matches the resolved request.
/// Broadcast on race resolution so non-winning UIs hide the dialog.
pub(super) fn tool_permission_resolved(state: &mut SessionViewState, id: &str) -> MutationEffect {
    let pending = &mut state.agent.conversation.pending_permission;
    if pending.as_ref().is_some_and(|p| p.id == id) {
        *pending = None;
        MutationEffect::Mutated
    } else {
        MutationEffect::NoOp
    }
}

pub(super) fn plan_approval_request(
    state: &mut SessionViewState,
    id: &str,
    plan_content: &str,
    plan_path: &str,
) -> MutationEffect {
    let conv = &mut state.agent.conversation;
    conv.flush_streaming();
    conv.pending_plan_approval = Some(PendingPlanApproval {
        id: id.to_string(),
        plan_content: plan_content.to_string(),
        plan_path: plan_path.to_string(),
    });
    MutationEffect::Mutated
}

pub(super) fn plan_approval_resolved(state: &mut SessionViewState, id: &str) -> MutationEffect {
    let pending = &mut state.agent.conversation.pending_plan_approval;
    if pending.as_ref().is_some_and(|value| value.id == id) {
        *pending = None;
        MutationEffect::Mutated
    } else {
        MutationEffect::NoOp
    }
}

pub(super) fn user_message_queued(
    state: &mut SessionViewState,
    message_id: &str,
    content: &str,
    image_count: usize,
    skill_info: Option<loopal_protocol::SkillInvocation>,
) -> MutationEffect {
    if message_seen(state, message_id) {
        return MutationEffect::NoOp;
    }
    let mut text = content.to_string();
    if image_count > 0 {
        text.push_str(&format!(" [+{image_count} image(s)]"));
    }
    state.agent.conversation.messages.push(SessionMessage {
        role: "user".to_string(),
        content: text,
        image_count,
        skill_info,
        message_id: Some(message_id.to_string()),
        ..Default::default()
    });
    MutationEffect::Mutated
}

pub(super) fn auto_continuation(
    state: &mut SessionViewState,
    cont: u32,
    max: u32,
    reason: &str,
) -> MutationEffect {
    conversation_display::handle_auto_continuation(
        &mut state.agent.conversation,
        cont,
        max,
        reason,
    );
    MutationEffect::Mutated
}

pub(super) fn compacted(
    state: &mut SessionViewState,
    kept: usize,
    summarized: usize,
    tokens_before: u32,
    tokens_after: u32,
    strategy: &str,
) -> MutationEffect {
    state.agent.conversation.compact_banner = None;
    conversation_display::handle_compaction(
        &mut state.agent.conversation,
        kept,
        summarized,
        tokens_before,
        tokens_after,
        strategy,
    );
    MutationEffect::Mutated
}

pub(super) fn inbox_enqueued(
    state: &mut SessionViewState,
    message_id: &str,
    source: &MessageSource,
    content: &str,
    summary: Option<&str>,
) -> MutationEffect {
    if source.is_ephemeral_in_history() {
        return MutationEffect::NoOp;
    }
    if message_seen(state, message_id) {
        return MutationEffect::NoOp;
    }
    conversation_display::push_inbox_msg(
        &mut state.agent.conversation,
        message_id.to_string(),
        source.clone(),
        content.to_string(),
        summary.map(String::from),
    );
    MutationEffect::Mutated
}

fn message_seen(state: &SessionViewState, message_id: &str) -> bool {
    state.agent.conversation.messages.iter().any(|message| {
        message.message_id.as_deref() == Some(message_id)
            || message
                .inbox
                .as_ref()
                .is_some_and(|inbox| inbox.message_id == message_id)
    })
}

pub(super) fn permission_decided(
    state: &mut SessionViewState,
    tool_name: &str,
    decision: &str,
    reason: &str,
    duration_ms: u64,
) -> MutationEffect {
    if decision == "allow" {
        return MutationEffect::NoOp;
    }
    let label = if decision == "deny" {
        "permission denied"
    } else {
        "permission"
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
    MutationEffect::Mutated
}
