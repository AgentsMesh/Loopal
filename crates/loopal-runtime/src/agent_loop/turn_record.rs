use loopal_turn::{
    OrderedToolBatch, ToolBatchItem, ToolCall, ToolCallId, ToolExecState, TurnEvent, TurnId,
    TurnOutcome, TurnStep, TurnTrigger,
};
use tracing::warn;

use super::runner::AgentLoopRunner;
use super::turn_tracker::TurnEventLogger;
use crate::session::SessionManager;

// reason: persistence sink — TurnTracker emits TurnEvent through this logger,
// which writes turns.jsonl. Failure returns false; TurnTracker rolls back
// in-memory state to keep both views in lockstep.
struct JsonlLogger<'a> {
    sm: &'a SessionManager,
    session_id: &'a str,
}

impl TurnEventLogger for JsonlLogger<'_> {
    fn persist(&self, event: &TurnEvent) -> bool {
        match self.sm.record_turn_event(self.session_id, event) {
            Ok(()) => true,
            Err(e) => {
                warn!(error = %e, "record_turn_event persist failed; rolling back in-memory update");
                false
            }
        }
    }
}

impl AgentLoopRunner {
    pub fn start_turn_record(&mut self, trigger: TurnTrigger) -> Option<TurnId> {
        let logger = JsonlLogger {
            sm: &self.params.deps.session_manager,
            session_id: &self.params.session.id,
        };
        let id = self.turns.try_start_turn(trigger, &logger);
        if id.is_some() {
            self.refresh_context_view();
        }
        id
    }

    pub(super) fn append_step_record(
        &mut self,
        step: TurnStep,
    ) -> Result<u32, super::turn_tracker::TurnTrackerError> {
        let logger = JsonlLogger {
            sm: &self.params.deps.session_manager,
            session_id: &self.params.session.id,
        };
        let idx = self.turns.try_append_step(step, &logger)?;
        self.refresh_context_view();
        Ok(idx)
    }

    /// Open a ToolBatch step in Pending state, carrying the full ToolCall info
    /// (name + input). Returns `Ok(Some(step_index))` on success, `Ok(None)`
    /// when `tool_uses` is empty (no-op), or `Err` when TurnStore write or
    /// event persist fails.
    pub(super) fn start_tool_batch_record(
        &mut self,
        tool_uses: &[(String, String, serde_json::Value)],
    ) -> Result<Option<u32>, super::turn_tracker::TurnTrackerError> {
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
        let logger = JsonlLogger {
            sm: &self.params.deps.session_manager,
            session_id: &self.params.session.id,
        };
        if let Err(e) = self
            .turns
            .try_update_tool_state(item_index, new_state, &logger)
        {
            warn!(error = %e, item_index, "update_tool_batch_item_state failed; turn step left at prior state");
            return;
        }
        self.refresh_context_view();
    }

    pub(super) fn close_tool_batch_record(&mut self) {
        self.turns.close_tool_batch();
    }

    pub(super) fn end_turn_record(&mut self, outcome: TurnOutcome) {
        let logger = JsonlLogger {
            sm: &self.params.deps.session_manager,
            session_id: &self.params.session.id,
        };
        self.turns.try_end_turn(outcome, &logger);
        self.refresh_context_view();
    }

    fn refresh_context_view(&mut self) {
        let turns = self.turns.store().turns();
        self.params.store.refresh_view(turns);
    }
}
