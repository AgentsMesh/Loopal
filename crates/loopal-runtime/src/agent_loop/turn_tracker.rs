use chrono::Utc;
use loopal_context::TurnStore;
use loopal_turn::{TurnEvent, TurnId, TurnOutcome, TurnStep, TurnTrigger};
use tracing::warn;

/// Persistence sink for `TurnEvent`s. Returns `true` on success. Implementations
/// MUST write to the durable log before returning `true`; on `false`,
/// `TurnTracker` rolls back the corresponding in-memory mutation so the two
/// views stay in lockstep (fail-closed atomicity).
pub trait TurnEventLogger {
    fn persist(&self, event: &TurnEvent) -> bool;
}

/// Domain-layer turn tracking state — owned exclusively by `AgentLoopRunner`.
/// All mutating methods are inherent on this type so the single-writer
/// contract is enforced by visibility (fields are crate-private).
pub struct TurnTracker {
    current_turn_id: Option<TurnId>,
    current_step_index: u32,
    current_tool_batch_step: Option<u32>,
    store: TurnStore,
}

impl TurnTracker {
    pub fn new(store: TurnStore) -> Self {
        Self {
            current_turn_id: None,
            current_step_index: 0,
            current_tool_batch_step: None,
            store,
        }
    }

    // ── Read-only accessors ────────────────────────────────────────────────
    pub fn current_turn_id(&self) -> Option<&TurnId> {
        self.current_turn_id.as_ref()
    }
    pub fn current_tool_batch_step(&self) -> Option<u32> {
        self.current_tool_batch_step
    }
    pub fn store(&self) -> &TurnStore {
        &self.store
    }
    pub fn store_mut(&mut self) -> &mut TurnStore {
        &mut self.store
    }

    /// Replace the inner `TurnStore` (e.g. on session resume) and reset the
    /// tracker's in-flight pointers to match. Tool-batch and step indices
    /// derive from the new store: `current_turn_id` mirrors the store's last
    /// InProgress turn (if any); step/tool counters reset to 0/None.
    pub fn replace_store(&mut self, store: TurnStore) {
        self.current_turn_id = store.current_turn_id().cloned();
        let step_count = store
            .current_turn()
            .map(|t| t.body.steps.len() as u32)
            .unwrap_or(0);
        self.current_step_index = step_count;
        self.current_tool_batch_step = None;
        self.store = store;
    }

    // ── Mutators (fail-closed: persist before in-memory commit) ────────────

    /// Open a new turn. Persists `TurnStarted` to the log; on persist failure
    /// rolls back the in-memory push so `store` and the log stay aligned.
    pub fn try_start_turn(
        &mut self,
        trigger: TurnTrigger,
        logger: &dyn TurnEventLogger,
    ) -> Option<TurnId> {
        let id = self.store.start_turn(trigger.clone());
        let event = TurnEvent::TurnStarted {
            turn_id: id.clone(),
            started_at: Utc::now(),
            trigger,
        };
        if logger.persist(&event) {
            self.current_turn_id = Some(id.clone());
            self.current_step_index = 0;
            self.current_tool_batch_step = None;
            Some(id)
        } else {
            self.store.turns_mut().pop();
            None
        }
    }

    /// Append a step to the current turn. Returns the assigned `step_index` on
    /// success; rolls back the in-memory push on persist failure.
    pub fn try_append_step(&mut self, step: TurnStep, logger: &dyn TurnEventLogger) -> Option<u32> {
        let turn_id = self.current_turn_id.as_ref()?.clone();
        let step_index = self.current_step_index;
        if let Err(e) = self.store.append_step(step.clone()) {
            warn!(error = %e, "turn_store append_step failed; skipping event persist");
            return None;
        }
        let event = TurnEvent::StepAppended {
            turn_id: turn_id.clone(),
            step_index,
            step,
        };
        if logger.persist(&event) {
            self.current_step_index += 1;
            Some(step_index)
        } else {
            if let Some(turn) = self.store.turns_mut().iter_mut().find(|t| t.id == turn_id) {
                turn.body.steps.pop();
            }
            None
        }
    }

    /// Mark the just-appended step as the in-flight ToolBatch so subsequent
    /// `try_update_tool_state` calls can patch its items.
    pub fn mark_tool_batch_open(&mut self, step_index: u32) {
        self.current_tool_batch_step = Some(step_index);
    }

    pub fn close_tool_batch(&mut self) {
        self.current_tool_batch_step = None;
    }

    /// Patch an in-flight ToolBatch item's state. Snapshots the prior state so
    /// a persist failure can roll back in-memory.
    pub fn try_update_tool_state(
        &mut self,
        item_index: u32,
        new_state: loopal_turn::ToolExecState,
        logger: &dyn TurnEventLogger,
    ) {
        let Some(turn_id) = self.current_turn_id.clone() else {
            return;
        };
        let Some(step_index) = self.current_tool_batch_step else {
            warn!("try_update_tool_state called without in-flight ToolBatch step");
            return;
        };
        let old_state = self
            .store
            .current_turn()
            .and_then(|t| t.body.steps.get(step_index as usize))
            .and_then(|s| match s {
                TurnStep::ToolBatch(b) => b.items.get(item_index as usize).map(|i| i.state.clone()),
                _ => None,
            });
        if let Err(e) = self
            .store
            .update_tool_state(step_index, item_index, new_state.clone())
        {
            warn!(error = %e, "turn_store update_tool_state failed; skipping event persist");
            return;
        }
        let event = TurnEvent::StepUpdated {
            turn_id,
            step_index,
            item_index,
            new_state,
        };
        if !logger.persist(&event)
            && let Some(prev) = old_state
        {
            let _ = self.store.update_tool_state(step_index, item_index, prev);
        }
    }

    /// Close the current turn. Persists `TurnEnded`; if the persist fails, the
    /// in-memory turn is still marked ended — resume from log will see the
    /// missing `TurnEnded` and apply `CrashRecovery` Cancelled semantics, so
    /// both views converge.
    pub fn try_end_turn(&mut self, outcome: TurnOutcome, logger: &dyn TurnEventLogger) {
        let Some(turn_id) = self.current_turn_id.clone() else {
            return;
        };
        if let Err(e) = self.store.end_current_turn(outcome.clone()) {
            warn!(error = %e, "turn_store end_current_turn failed");
            return;
        }
        let event = TurnEvent::TurnEnded {
            turn_id: turn_id.clone(),
            outcome,
        };
        if !logger.persist(&event) {
            warn!(
                ?turn_id,
                "TurnEnded event persist failed; in-memory ended, jsonl missing"
            );
        }
        self.current_turn_id = None;
        self.current_step_index = 0;
        self.current_tool_batch_step = None;
    }
}
