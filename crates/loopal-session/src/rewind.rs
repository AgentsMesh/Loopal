//! Display-side rewind: truncate SessionState messages to match runtime state.

use crate::state::SessionState;
use crate::types::DisplayMessage;

/// Truncate display messages to retain only the first `remaining_turns` user turns.
///
/// A "turn" in the display layer is a user message followed by its assistant responses
/// and tool calls. We count user-role display messages to find the truncation point.
pub fn truncate_display_to_turn(state: &mut SessionState, remaining_turns: usize) {
    if remaining_turns == 0 {
        state.messages.clear();
        state.streaming_text.clear();
        state.turn_count = 0;
        state.reset_timer();
        return;
    }

    let cut = find_display_cut_index(&state.messages, remaining_turns);
    state.messages.truncate(cut);
    state.streaming_text.clear();
    state.turn_count = remaining_turns as u32;
}

/// Find the index of the first display message belonging to turn N+1
/// (i.e., the Nth user message, 0-indexed), so we can truncate there.
fn find_display_cut_index(messages: &[DisplayMessage], remaining_turns: usize) -> usize {
    let mut user_count = 0;
    for (i, msg) in messages.iter().enumerate() {
        if msg.role == "user" {
            user_count += 1;
            if user_count > remaining_turns {
                return i;
            }
        }
    }
    // All messages belong to the retained turns
    messages.len()
}
