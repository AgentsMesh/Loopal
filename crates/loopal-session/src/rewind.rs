//! Display-side rewind: truncate conversation messages to match runtime state.

use crate::agent_conversation::AgentConversation;
use crate::types::SessionMessage;

/// Truncate display messages to retain only the first `remaining_turns` user turns.
///
/// A "turn" in the display layer is a user message followed by its assistant responses
/// and tool calls. We count user-role display messages to find the truncation point.
pub fn truncate_display_to_turn(conv: &mut AgentConversation, remaining_turns: usize) {
    if remaining_turns == 0 {
        conv.messages.clear();
        conv.streaming_text.clear();
        conv.turn_count = 0;
        conv.reset_timer();
        return;
    }

    let cut = find_display_cut_index(&conv.messages, remaining_turns);
    conv.messages.truncate(cut);
    conv.streaming_text.clear();
    conv.turn_count = remaining_turns as u32;
}

/// Find the index of the first display message belonging to turn N+1
/// (i.e., the Nth user message, 0-indexed), so we can truncate there.
fn find_display_cut_index(messages: &[SessionMessage], remaining_turns: usize) -> usize {
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
