use loopal_context::{PersistError, TurnEventLogger, TurnTrackerError};
use loopal_turn::{
    OrderedToolBatch, ToolBatchItem, ToolCall, ToolCallId, ToolExecState, TurnEvent, TurnId,
    TurnOutcome, TurnStep, TurnTrigger,
};
use tracing::warn;

use super::runner::AgentLoopRunner;
use crate::session::SessionManager;

struct JsonlLogger<'a> {
    sm: &'a SessionManager,
    session_id: &'a str,
}

impl TurnEventLogger for JsonlLogger<'_> {
    fn persist(&self, event: &TurnEvent) -> Result<(), PersistError> {
        self.sm
            .record_turn_event(self.session_id, event)
            .map_err(|e| PersistError(e.to_string()))
    }
}

// Free fn (not a method) so the compiler sees the disjoint field borrows
// — `&self.params.X` vs `&mut self.turns` — and allows them to coexist.
// A `self.jsonl_logger()` method would borrow all of `self`.
fn make_logger<'a>(sm: &'a SessionManager, session_id: &'a str) -> JsonlLogger<'a> {
    JsonlLogger { sm, session_id }
}

impl AgentLoopRunner {
    pub fn start_turn_record(&mut self, trigger: TurnTrigger) -> Option<TurnId> {
        let logger = make_logger(&self.params.deps.session_manager, &self.params.session.id);
        self.turns.try_start_turn(trigger, &logger)
    }

    pub fn append_step_record(&mut self, step: TurnStep) -> Result<u32, TurnTrackerError> {
        let logger = make_logger(&self.params.deps.session_manager, &self.params.session.id);
        self.turns.try_append_step(step, &logger)
    }

    pub(super) fn start_tool_batch_record(
        &mut self,
        tool_uses: &[(String, String, serde_json::Value)],
    ) -> Result<Option<u32>, TurnTrackerError> {
        if tool_uses.is_empty() {
            return Ok(None);
        }
        let items: Vec<ToolBatchItem> = tool_uses
            .iter()
            .map(|(id, name, input)| ToolBatchItem {
                call: ToolCall {
                    id: ToolCallId::new(id),
                    name: name.clone(),
                    input: input.clone(),
                },
                state: ToolExecState::Pending,
            })
            .collect();
        let step_index =
            self.append_step_record(TurnStep::ToolBatch(OrderedToolBatch { items }))?;
        self.turns.mark_tool_batch_open(step_index);
        Ok(Some(step_index))
    }

    pub(super) fn update_tool_batch_item_state(
        &mut self,
        item_index: u32,
        new_state: ToolExecState,
    ) {
        let logger = make_logger(&self.params.deps.session_manager, &self.params.session.id);
        if let Err(e) = self
            .turns
            .try_update_tool_state(item_index, new_state, &logger)
        {
            warn!(error = %e, item_index, "update_tool_batch_item_state failed");
        }
    }

    pub(super) fn close_tool_batch_record(&mut self) {
        self.turns.close_tool_batch();
    }

    pub(super) fn cancel_open_tool_batch_record(&mut self, cause: loopal_turn::CancelCause) {
        let logger = make_logger(&self.params.deps.session_manager, &self.params.session.id);
        self.turns.cancel_open_tool_batch(cause, &logger);
    }

    // reason: user-level cancellation MUST go through finalize_turn_cancellation
    // (pairs tool_use/tool_result, resets continuation, emits TurnCancelled).
    // Two paths intentionally bypass finalize and end here directly:
    //   1. compaction's synthetic host turn (compaction/host.rs) — owns no tool
    //      batch and no continuation state.
    //   2. governance abort (turn_observer_dispatch.rs) — the turn made a real
    //      LlmCall and ends Complete with Cancelled compensation items; it must
    //      NOT call on_turn_cancelled (that would clear the very loop/degeneration
    //      streak that triggered the abort) and must NOT emit TurnCancelled.
    pub fn end_turn_record(&mut self, outcome: TurnOutcome) {
        let logger = make_logger(&self.params.deps.session_manager, &self.params.session.id);
        if let Err(e) = self.turns.end_turn(outcome, &logger) {
            warn!(error = %e, "end_turn failed; in-memory state unchanged");
        }
    }

    pub(super) fn clear_turns_record(&mut self) {
        let logger = make_logger(&self.params.deps.session_manager, &self.params.session.id);
        self.turns.clear(&logger);
    }

    pub(super) fn rewind_turns_record(&mut self, keep: usize) {
        let logger = make_logger(&self.params.deps.session_manager, &self.params.session.id);
        self.turns.rewind(keep, &logger);
    }

    pub fn seed_test_turns(&mut self, turns: Vec<loopal_turn::Turn>) {
        // InProgress turns must be at the END of the vec only — a mid-list
        // InProgress would block subsequent start_turn_record via F7 guard,
        // silently dropping every later turn.
        if let Some((mid_idx, _)) = turns
            .iter()
            .enumerate()
            .rev()
            .skip(1)
            .find(|(_, t)| matches!(t.outcome, TurnOutcome::InProgress))
        {
            panic!(
                "seed_test_turns: InProgress turn at index {mid_idx} is not the last in the input vec — \
                 only the trailing turn may remain InProgress",
            );
        }
        for turn in turns {
            if self.start_turn_record(turn.trigger).is_none() {
                continue;
            }
            for step in turn.body.steps {
                self.append_step_record(step)
                    .expect("seed_test_turns: append_step_record must succeed");
            }
            if !matches!(turn.outcome, TurnOutcome::InProgress) {
                self.end_turn_record(turn.outcome);
            }
        }
    }
}
