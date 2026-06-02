use std::time::{Duration, SystemTime, UNIX_EPOCH};

use loopal_turn::{Turn, TurnId, TurnStep, TurnTrigger};

use super::TurnStore;

impl TurnStore {
    pub fn turns(&self) -> &[Turn] {
        &self.turns
    }

    pub fn current_turn(&self) -> Option<&Turn> {
        let id = self.current_turn_id.as_ref()?;
        self.turns.iter().find(|t| &t.id == id)
    }

    pub fn current_turn_id(&self) -> Option<&TurnId> {
        self.current_turn_id.as_ref()
    }

    pub fn current_turn_index(&self) -> Option<usize> {
        let id = self.current_turn_id.as_ref()?;
        self.turns.iter().position(|t| &t.id == id)
    }

    pub fn len(&self) -> usize {
        self.turns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }

    /// Map a 0-based UserInput-turn ordinal to its TurnStore index, skipping
    /// synthetic triggers. Used by `/rewind` where the picker counts user
    /// messages but synthetic turns may be interleaved in the store.
    pub fn user_input_turn_index(&self, ordinal: usize) -> Option<usize> {
        self.turns
            .iter()
            .enumerate()
            .filter(|(_, t)| matches!(t.trigger, TurnTrigger::UserInput { .. }))
            .map(|(i, _)| i)
            .nth(ordinal)
    }

    /// Turn index of the first turn to KEEP during compaction. Policy: keep
    /// only the last turn (it carries the active user request + in-flight
    /// tool batch).
    pub fn compact_boundary_turn_index(&self) -> usize {
        const KEEP_TAIL_TURNS: usize = 1;
        self.turns.len().saturating_sub(KEEP_TAIL_TURNS)
    }

    /// Latest turn containing an LlmCall, used by microcompact's idle gauge.
    /// `last_step_at` is bumped on both append_step AND update_tool_state, so
    /// microcompact will NOT scrub mid-batch while items are still transitioning.
    /// Negative timestamps surface a `warn` (corruption signal) and the turn
    /// is skipped rather than clamped to UNIX_EPOCH (which would force a 56-yr
    /// idle reading).
    pub fn last_assistant_activity_at(&self) -> Option<SystemTime> {
        self.turns
            .iter()
            .rev()
            .find(|t| {
                t.body
                    .steps
                    .iter()
                    .any(|s| matches!(s, TurnStep::LlmCall { .. }))
            })
            .and_then(|t| {
                let dt = t.last_step_at.unwrap_or(t.started_at);
                let secs = dt.timestamp();
                if secs < 0 {
                    tracing::warn!(
                        turn_id = %t.id.as_str(),
                        timestamp = secs,
                        "negative timestamp in turn activity; skipping (corrupted data?)"
                    );
                    return None;
                }
                Some(UNIX_EPOCH + Duration::from_secs(secs as u64))
            })
    }
}
