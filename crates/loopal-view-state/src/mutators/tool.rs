use loopal_protocol::AgentStatus;

use crate::conversation::{server_tool_display, tool_result_handler};
use crate::state::SessionViewState;

use super::MutationEffect;

pub(super) fn tool_call(
    state: &mut SessionViewState,
    id: &str,
    name: &str,
    input: &serde_json::Value,
) -> MutationEffect {
    if id.is_empty() {
        return MutationEffect::NoOp;
    }
    let conv = &mut state.agent.conversation;
    conv.mark_active();
    let accepted = tool_result_handler::handle_tool_call(
        conv,
        id.to_string(),
        name.to_string(),
        input.clone(),
    );
    if !accepted {
        return MutationEffect::NoOp;
    }
    // Tool execution only begins after pre-call compaction has completed.
    conv.compact_banner = None;
    conv.retry_banner = None;
    state.agent.observable.status = AgentStatus::Running;
    MutationEffect::Mutated
}

pub(super) fn tool_result(
    state: &mut SessionViewState,
    id: &str,
    name: &str,
    result: &str,
    is_error: bool,
    metadata: Option<loopal_tool_invocation::ToolResultMetadata>,
) -> MutationEffect {
    let conv = &mut state.agent.conversation;
    conv.mark_active();
    let transitioned = tool_result_handler::handle_tool_result(
        conv,
        tool_result_handler::ToolResultParams {
            id: id.to_string(),
            name: name.to_string(),
            result: result.to_string(),
            is_error,
            metadata,
        },
    );
    if transitioned {
        conv.compact_banner = None;
        conv.retry_banner = None;
        state.agent.observable.status = AgentStatus::Running;
        MutationEffect::Mutated
    } else {
        MutationEffect::NoOp
    }
}

pub(super) fn tool_batch_start(
    state: &mut SessionViewState,
    tool_ids: &[String],
) -> MutationEffect {
    let conv = &mut state.agent.conversation;
    conv.compact_banner = None;
    conv.retry_banner = None;
    conv.mark_active();
    tool_result_handler::handle_tool_batch_start(conv, tool_ids);
    MutationEffect::Mutated
}

pub(super) fn tool_progress(
    state: &mut SessionViewState,
    id: &str,
    output_tail: &str,
) -> MutationEffect {
    let conv = &mut state.agent.conversation;
    conv.compact_banner = None;
    conv.retry_banner = None;
    conv.mark_active();
    tool_result_handler::handle_tool_progress(conv, id.to_string(), output_tail.to_string());
    MutationEffect::Mutated
}

pub(super) fn server_tool_use(
    state: &mut SessionViewState,
    id: &str,
    name: &str,
    input: &serde_json::Value,
) -> MutationEffect {
    let conv = &mut state.agent.conversation;
    conv.compact_banner = None;
    conv.retry_banner = None;
    conv.mark_active();
    server_tool_display::handle_server_tool_use(
        conv,
        id.to_string(),
        name.to_string(),
        input.clone(),
    );
    state.agent.observable.status = AgentStatus::Running;
    MutationEffect::Mutated
}

pub(super) fn server_tool_result(
    state: &mut SessionViewState,
    tool_use_id: &str,
    content: &serde_json::Value,
) -> MutationEffect {
    let conv = &mut state.agent.conversation;
    conv.compact_banner = None;
    conv.retry_banner = None;
    conv.mark_active();
    server_tool_display::handle_server_tool_result(conv, tool_use_id, content);
    state.agent.observable.status = AgentStatus::Running;
    MutationEffect::Mutated
}

pub(super) fn server_tool_discarded(
    state: &mut SessionViewState,
    tool_use_id: &str,
    reason: loopal_tool_invocation::StaleReason,
) -> MutationEffect {
    let conv = &mut state.agent.conversation;
    conv.compact_banner = None;
    conv.retry_banner = None;
    conv.mark_active();
    server_tool_display::handle_server_tool_discarded(conv, tool_use_id, reason);
    state.agent.observable.status = AgentStatus::Running;
    MutationEffect::Mutated
}
