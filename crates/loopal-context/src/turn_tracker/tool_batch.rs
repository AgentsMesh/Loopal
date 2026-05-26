use loopal_turn::{ToolExecState, TurnEvent, TurnStep};

use super::TurnTracker;
use super::error::TurnTrackerError;
use super::logger::TurnEventLogger;

impl TurnTracker {
    pub fn mark_tool_batch_open(&mut self, step_index: u32) {
        self.current_tool_batch_step = Some(step_index);
    }

    pub fn close_tool_batch(&mut self) {
        self.current_tool_batch_step = None;
    }

    pub fn try_update_tool_state(
        &mut self,
        item_index: u32,
        new_state: ToolExecState,
        logger: &dyn TurnEventLogger,
    ) -> Result<(), TurnTrackerError> {
        let turn_id = self
            .store
            .current_turn_id()
            .cloned()
            .ok_or(TurnTrackerError::NoCurrentTurn)?;
        let step_index = self
            .current_tool_batch_step
            .ok_or(TurnTrackerError::NoToolBatchOpen)?;
        let old_state = self
            .store
            .current_turn()
            .and_then(|t| t.body.steps.get(step_index as usize))
            .and_then(|s| match s {
                TurnStep::ToolBatch(b) => b.items.get(item_index as usize).map(|i| i.state.clone()),
                _ => None,
            });
        self.store
            .update_tool_state(step_index, item_index, new_state.clone())?;
        let event = TurnEvent::StepUpdated {
            turn_id,
            step_index,
            item_index,
            new_state,
            updated_at: Some(chrono::Utc::now()),
        };
        if let Err(e) = logger.persist(&event) {
            if let Some(prev) = old_state {
                // reason: rollback can only fail if the step layout changed
                // between the original update and now (e.g. a wire_mut
                // replaced steps), which would imply &mut self aliasing
                // — impossible under Rust's borrow rules. log warn so
                // a future regression that breaks this invariant is
                // visible instead of silently producing a three-way
                // divergence (JSONL / store / view).
                if let Err(rollback_err) =
                    self.store.update_tool_state(step_index, item_index, prev)
                {
                    tracing::warn!(
                        error = %rollback_err,
                        step_index,
                        item_index,
                        "tool-state rollback failed after persist error; view may drift from JSONL"
                    );
                }
            }
            self.view.refresh_view(self.store.turns());
            return Err(TurnTrackerError::PersistFailed(e));
        }
        self.view.refresh_view(self.store.turns());
        Ok(())
    }
}
