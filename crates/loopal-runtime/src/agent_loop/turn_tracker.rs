use chrono::Utc;
use loopal_context::{TurnStore, TurnStoreError};
use loopal_turn::{TurnEvent, TurnId, TurnOutcome, TurnStep, TurnTrigger};
use tracing::warn;

/// Persistence sink for `TurnEvent`s. Returns `true` on success. Implementations
/// MUST write to the durable log before returning `true`; on `false`,
/// `TurnTracker` rolls back the corresponding in-memory mutation so the two
/// views stay in lockstep (fail-closed atomicity).
pub trait TurnEventLogger {
    fn persist(&self, event: &TurnEvent) -> bool;
}

/// Errors observable from `TurnTracker` mutators. Lets callers react to
/// failures (programmer error, store invariant, persist failure) uniformly
/// instead of swallowing them.
#[derive(Debug)]
pub enum TurnTrackerError {
    NoCurrentTurn,
    NoToolBatchOpen,
    Store(TurnStoreError),
    /// Persist failure surfaced through the same Result channel so callers
    /// can react uniformly. The in-memory mutation has already been rolled
    /// back when this is returned.
    PersistFailed,
}

impl std::fmt::Display for TurnTrackerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoCurrentTurn => write!(f, "no turn in progress"),
            Self::NoToolBatchOpen => {
                write!(
                    f,
                    "no in-flight ToolBatch step (call mark_tool_batch_open first)"
                )
            }
            Self::Store(e) => write!(f, "turn store error: {e}"),
            Self::PersistFailed => {
                write!(f, "event log persist failed; in-memory rolled back")
            }
        }
    }
}

impl std::error::Error for TurnTrackerError {}

impl From<TurnStoreError> for TurnTrackerError {
    fn from(e: TurnStoreError) -> Self {
        Self::Store(e)
    }
}

/// Fail-closed persistence adapter around `TurnStore`. Each mutator method
/// applies the in-memory change, then persists the matching `TurnEvent`; on
/// persist failure it rolls back via the store's `rollback_*` API so the
/// in-memory view and `turns.jsonl` stay in lockstep.
///
/// State is intentionally delegated: `current_turn_id` / step index come from
/// `TurnStore`. The only tracker-owned field is `current_tool_batch_step`,
/// which is transient (in-flight tool-batch marker, never persisted).
pub struct TurnTracker {
    store: TurnStore,
    current_tool_batch_step: Option<u32>,
}

impl TurnTracker {
    pub fn new(store: TurnStore) -> Self {
        Self {
            store,
            current_tool_batch_step: None,
        }
    }

    // ── Read-only accessors ────────────────────────────────────────────────
    pub fn current_turn_id(&self) -> Option<&TurnId> {
        self.store.current_turn_id()
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

    /// Replace the inner `TurnStore` (e.g. on session resume). The new store's
    /// `current_turn_id` (derived from its last InProgress turn) becomes the
    /// authoritative pointer; the transient `current_tool_batch_step` resets
    /// — in-flight tool batches never survive across a resume.
    pub fn replace_store(&mut self, store: TurnStore) {
        self.store = store;
        self.current_tool_batch_step = None;
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
            Some(id)
        } else {
            self.store.rollback_last_turn(&id);
            None
        }
    }

    /// Append a step to the current turn. Returns the assigned `step_index` on
    /// success; rolls back the in-memory push on persist failure.
    pub fn try_append_step(&mut self, step: TurnStep, logger: &dyn TurnEventLogger) -> Option<u32> {
        let turn_id = self.store.current_turn_id()?.clone();
        let step_index = match self.store.append_step(step.clone()) {
            Ok(idx) => idx,
            Err(e) => {
                warn!(error = %e, "turn_store append_step failed; skipping event persist");
                return None;
            }
        };
        let event = TurnEvent::StepAppended {
            turn_id,
            step_index,
            step,
        };
        if logger.persist(&event) {
            Some(step_index)
        } else {
            self.store.rollback_last_step();
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
    /// a persist failure can roll back in-memory. Returns:
    /// - `Ok(())` on success.
    /// - `Err(NoCurrentTurn / NoToolBatchOpen)` when caller precondition failed
    ///   (programmer error: turn never started or batch never marked open).
    /// - `Err(Store(_))` if the underlying store mutation failed (bad indices).
    /// - `Err(PersistFailed)` if event persist failed; in-memory has been
    ///   rolled back to the prior state.
    pub fn try_update_tool_state(
        &mut self,
        item_index: u32,
        new_state: loopal_turn::ToolExecState,
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
        };
        if !logger.persist(&event) {
            if let Some(prev) = old_state {
                let _ = self.store.update_tool_state(step_index, item_index, prev);
            }
            return Err(TurnTrackerError::PersistFailed);
        }
        Ok(())
    }

    /// Close the current turn. Persists `TurnEnded`; if the persist fails, the
    /// in-memory turn is still marked ended — resume from log will see the
    /// missing `TurnEnded` and apply `CrashRecovery` Cancelled semantics, so
    /// both views converge.
    pub fn try_end_turn(&mut self, outcome: TurnOutcome, logger: &dyn TurnEventLogger) {
        let Some(turn_id) = self.store.current_turn_id().cloned() else {
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
        self.current_tool_batch_step = None;
    }
}
