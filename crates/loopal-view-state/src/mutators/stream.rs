use loopal_protocol::AgentStatus;

use crate::state::SessionViewState;

pub(super) fn stream(state: &mut SessionViewState, text: &str) -> bool {
    let conv = &mut state.agent.conversation;
    conv.begin_turn();
    conv.mark_active();
    conv.streaming_text.push_str(text);
    state.agent.observable.status = AgentStatus::Running;
    true
}

pub(super) fn thinking_stream(state: &mut SessionViewState, text: &str) -> bool {
    let conv = &mut state.agent.conversation;
    conv.begin_turn();
    conv.mark_active();
    conv.thinking_active = true;
    conv.streaming_thinking.push_str(text);
    state.agent.observable.status = AgentStatus::Running;
    true
}

pub(super) fn thinking_complete(state: &mut SessionViewState, token_count: u32) -> bool {
    let conv = &mut state.agent.conversation;
    conv.mark_active();
    crate::conversation::thinking_display::handle_thinking_complete(conv, token_count);
    true
}

pub(super) fn retry_error(
    state: &mut SessionViewState,
    message: &str,
    attempt: u32,
    max: u32,
) -> bool {
    let conv = &mut state.agent.conversation;
    conv.retry_banner = Some(format!("{message} ({attempt}/{max})"));
    conv.mark_active();
    state.agent.observable.status = AgentStatus::Running;
    true
}

pub(super) fn retry_cleared(state: &mut SessionViewState) -> bool {
    state.agent.conversation.retry_banner = None;
    true
}

pub(super) fn rewound(state: &mut SessionViewState, remaining_turns: usize) -> bool {
    let conv = &mut state.agent.conversation;
    if remaining_turns == 0 {
        conv.messages.clear();
        conv.streaming_text.clear();
        conv.turn_count = 0;
        conv.reset_timer();
        return true;
    }
    let cut = find_display_cut_index(&conv.messages, remaining_turns);
    conv.messages.truncate(cut);
    conv.streaming_text.clear();
    conv.turn_count = remaining_turns as u32;
    true
}

fn find_display_cut_index(messages: &[crate::SessionMessage], remaining_turns: usize) -> usize {
    let mut user_count = 0;
    for (i, msg) in messages.iter().enumerate() {
        if msg.role == "user" {
            user_count += 1;
            if user_count > remaining_turns {
                return i;
            }
        }
    }
    messages.len()
}
