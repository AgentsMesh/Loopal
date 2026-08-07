use std::time::Instant;

use loopal_protocol::AgentStatus;

use crate::state::SessionViewState;

use super::MutationEffect;

pub(super) fn started(state: &mut SessionViewState) -> MutationEffect {
    state.agent.observable.status = AgentStatus::Running;
    state.agent.conversation.mark_active();
    ensure_started_at(state);
    MutationEffect::Mutated
}

pub(super) fn running(state: &mut SessionViewState) -> MutationEffect {
    state.agent.observable.status = AgentStatus::Running;
    // A new running phase cannot belong to an older compaction lifecycle.
    // This also repairs snapshots captured after a start event whose terminal
    // progress event was lost during a process or transport failure.
    state.agent.conversation.compact_banner = None;
    state.agent.conversation.retry_banner = None;
    state.agent.conversation.begin_turn();
    state.agent.conversation.mark_active();
    ensure_started_at(state);
    MutationEffect::Mutated
}

pub(super) fn awaiting_input(state: &mut SessionViewState) -> MutationEffect {
    set_idle(state, AgentStatus::WaitingForInput);
    MutationEffect::MutatedEndedTurn
}

pub(super) fn finished(state: &mut SessionViewState) -> MutationEffect {
    if state.agent.observable.status == AgentStatus::Error {
        return MutationEffect::NoOp;
    }
    set_idle(state, AgentStatus::Finished);
    MutationEffect::MutatedEndedTurn
}

pub(super) fn interrupted(state: &mut SessionViewState) -> MutationEffect {
    set_idle(state, AgentStatus::WaitingForInput);
    MutationEffect::MutatedEndedTurn
}

pub(super) fn error(state: &mut SessionViewState, message: &str) -> MutationEffect {
    let conv = &mut state.agent.conversation;
    conv.flush_streaming();
    conv.retry_banner = None;
    conv.compact_banner = None;
    conv.messages.push(crate::SessionMessage {
        role: "error".into(),
        content: message.into(),
        ..Default::default()
    });
    state.agent.observable.status = AgentStatus::Error;
    MutationEffect::MutatedEndedTurn
}

pub(super) fn provider_warning(state: &mut SessionViewState, message: &str) -> MutationEffect {
    let conv = &mut state.agent.conversation;
    conv.flush_streaming();
    crate::conversation::conversation_display::push_system_msg(conv, message);
    MutationEffect::Mutated
}

pub(super) fn token_usage(
    state: &mut SessionViewState,
    input: u32,
    output: u32,
    context_window: u32,
    cache_creation: u32,
    cache_read: u32,
) -> MutationEffect {
    let obs = &mut state.agent.observable;
    if input > 0 || cache_creation > 0 || cache_read > 0 {
        obs.input_tokens = input;
        obs.output_tokens = output;
    } else {
        obs.output_tokens = output;
    }
    crate::conversation::conversation_display::handle_token_usage(
        &mut state.agent.conversation,
        input,
        output,
        context_window,
        cache_creation,
        cache_read,
    );
    MutationEffect::Mutated
}

pub(super) fn turn_completed(state: &mut SessionViewState) -> MutationEffect {
    let agent = &mut state.agent;
    agent.conversation.turn_count = agent.conversation.turn_count.saturating_add(1);
    agent.observable.turn_count = agent.observable.turn_count.saturating_add(1);
    MutationEffect::MutatedEndedTurn
}

fn set_idle(state: &mut SessionViewState, status: AgentStatus) {
    let conv = &mut state.agent.conversation;
    conv.flush_streaming();
    conv.end_turn();
    conv.retry_banner = None;
    conv.compact_banner = None;
    state.agent.observable.status = status;
}

fn ensure_started_at(state: &mut SessionViewState) {
    if state.agent.started_at.is_none() {
        state.agent.started_at = Some(Instant::now());
    }
}
