//! Helpers split out from `agent_handler::apply_agent_event` — per-event
//! branches that previously inflated the main match expression past the
//! 200-line soft limit.
//!
//! Each helper accepts the whole `AgentEventPayload` but makes the contract
//! explicit via `let … else { unreachable!() }`: the dispatcher in
//! `agent_handler::apply_agent_event` is the only caller, and it has
//! already matched the variant. Passing any other variant is a programming
//! error and panics in debug builds rather than silently no-op'ing.

use loopal_protocol::{AgentEventPayload, AgentStatus, ObservableAgentState};

use crate::agent_conversation::AgentConversation;
use crate::conversation_display::{
    handle_auto_continuation, handle_compaction, handle_token_usage, push_system_msg,
};
use crate::state::SessionState;
use crate::tool_result_handler::{ToolResultParams, handle_tool_result};
use crate::types::{PendingPermission, PendingQuestion, SessionMessage};

pub(crate) fn apply_tool_result_event(
    conv: &mut AgentConversation,
    obs: &mut ObservableAgentState,
    payload: AgentEventPayload,
) {
    let AgentEventPayload::ToolResult {
        id,
        name,
        result,
        is_error,
        duration_ms,
        metadata,
    } = payload
    else {
        unreachable!("apply_tool_result_event called with non-ToolResult variant");
    };
    handle_tool_result(
        conv,
        ToolResultParams {
            id,
            name,
            result,
            is_error,
            duration_ms,
            metadata,
        },
    );
    obs.tools_in_flight = obs.tools_in_flight.saturating_sub(1);
    obs.status = AgentStatus::Running;
}

pub(crate) fn apply_tool_permission_request(
    conv: &mut AgentConversation,
    payload: AgentEventPayload,
) {
    let AgentEventPayload::ToolPermissionRequest { id, name, input } = payload else {
        unreachable!("apply_tool_permission_request called with wrong variant");
    };
    conv.flush_streaming();
    conv.pending_permission = Some(PendingPermission {
        id,
        name,
        input,
        relay_request_id: None,
    });
}

pub(crate) fn apply_user_question_request(
    conv: &mut AgentConversation,
    payload: AgentEventPayload,
) {
    let AgentEventPayload::UserQuestionRequest { id, questions } = payload else {
        unreachable!("apply_user_question_request called with wrong variant");
    };
    conv.flush_streaming();
    conv.pending_question = Some(PendingQuestion::new(id, questions));
}

pub(crate) fn apply_error_event(
    conv: &mut AgentConversation,
    obs: &mut ObservableAgentState,
    message: String,
) {
    conv.flush_streaming();
    conv.retry_banner = None;
    conv.messages.push(SessionMessage {
        role: "error".into(),
        content: message,
        tool_calls: Vec::new(),
        image_count: 0,
        skill_info: None,
        inbox: None,
    });
    obs.status = AgentStatus::Error;
}

pub(crate) fn apply_token_usage(
    conv: &mut AgentConversation,
    obs: &mut ObservableAgentState,
    payload: AgentEventPayload,
) {
    let AgentEventPayload::TokenUsage {
        input_tokens,
        output_tokens,
        context_window,
        cache_creation_input_tokens,
        cache_read_input_tokens,
        ..
    } = payload
    else {
        unreachable!("apply_token_usage called with wrong variant");
    };
    handle_token_usage(
        conv,
        input_tokens,
        output_tokens,
        context_window,
        cache_creation_input_tokens,
        cache_read_input_tokens,
    );
    obs.input_tokens = input_tokens;
    obs.output_tokens = output_tokens;
}

pub(crate) fn apply_compaction_event(conv: &mut AgentConversation, payload: AgentEventPayload) {
    let AgentEventPayload::Compacted {
        kept,
        removed,
        tokens_before,
        tokens_after,
        strategy,
    } = payload
    else {
        unreachable!("apply_compaction_event called with wrong variant");
    };
    handle_compaction(conv, kept, removed, tokens_before, tokens_after, &strategy);
}

pub(crate) fn apply_auto_continuation(conv: &mut AgentConversation, payload: AgentEventPayload) {
    let AgentEventPayload::AutoContinuation {
        continuation,
        max_continuations,
    } = payload
    else {
        unreachable!("apply_auto_continuation called with wrong variant");
    };
    handle_auto_continuation(conv, continuation, max_continuations);
}

pub(crate) fn apply_auto_mode_decision(conv: &mut AgentConversation, payload: AgentEventPayload) {
    let AgentEventPayload::AutoModeDecision {
        tool_name,
        decision,
        reason,
        duration_ms,
    } = payload
    else {
        unreachable!("apply_auto_mode_decision called with wrong variant");
    };
    let label = if decision == "allow" {
        "auto-allowed"
    } else {
        "auto-denied"
    };
    let t = if duration_ms > 0 {
        format!("({duration_ms}ms)")
    } else {
        "(cached)".into()
    };
    push_system_msg(conv, &format!("[{label}] {tool_name}: {reason} {t}"));
}

pub(crate) fn clear_all_panel_caches_for_resume(
    state: &mut SessionState,
    payload: AgentEventPayload,
) {
    crate::cron_state::apply(state, payload.clone());
    crate::task_state::apply(state, payload.clone());
    crate::bg_task_state::apply(state, payload);
}
