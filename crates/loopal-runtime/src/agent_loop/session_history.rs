use loopal_error::Result;
use loopal_protocol::{AgentEventPayload, ProjectedMessage, SessionHistorySnapshot};

use super::runner::AgentLoopRunner;

const MAX_HISTORY_MESSAGES: usize = 512;
const MAX_HISTORY_FRAME_BYTES: usize = 4 * 1024 * 1024;
const FRAME_ENVELOPE_RESERVE: usize = 16 * 1024;

impl AgentLoopRunner {
    pub(super) async fn emit_initial_session_history(&self) -> Result<()> {
        if !self.params.hydrate_initial_history {
            return Ok(());
        }
        self.emit_session_history().await
    }

    pub(super) async fn emit_session_history(&self) -> Result<()> {
        let messages = loopal_provider_api::project_turns_to_messages(self.turns.store().turns());
        let projected = loopal_context::project_messages_to_display(&messages);
        self.emit(AgentEventPayload::SessionHistoryLoaded(bounded_history(
            self.params.session.id.clone(),
            projected,
        )))
        .await
    }
}

fn bounded_history(session_id: String, messages: Vec<ProjectedMessage>) -> SessionHistorySnapshot {
    let original_len = messages.len();
    let skip = original_len.saturating_sub(MAX_HISTORY_MESSAGES);
    let mut snapshot = SessionHistorySnapshot {
        session_id,
        messages: Vec::new(),
        truncated: skip > 0,
    };
    let limit = MAX_HISTORY_FRAME_BYTES - FRAME_ENVELOPE_RESERVE;
    let mut used = serde_json::to_vec(&AgentEventPayload::SessionHistoryLoaded(snapshot.clone()))
        .map(|encoded| encoded.len())
        .unwrap_or(limit);
    for message in messages.into_iter().skip(skip).rev() {
        let size = serde_json::to_vec(&message)
            .map(|encoded| encoded.len() + usize::from(!snapshot.messages.is_empty()))
            .unwrap_or(usize::MAX);
        if size > limit.saturating_sub(used) {
            snapshot.truncated = true;
            break;
        }
        used += size;
        snapshot.messages.push(message);
    }
    snapshot.messages.reverse();
    snapshot.truncated |= snapshot.messages.len() < original_len;
    snapshot
}

#[cfg(test)]
#[path = "session_history_tests.rs"]
mod tests;
