use chrono::Utc;
use loopal_turn::{TurnEvent, TurnId, TurnOutcome, TurnStep, TurnTrigger};
use tracing::warn;

use super::TurnTracker;
use super::error::TurnTrackerError;
use super::logger::TurnEventLogger;

impl TurnTracker {
    pub fn try_start_turn(
        &mut self,
        trigger: TurnTrigger,
        logger: &dyn TurnEventLogger,
    ) -> Option<TurnId> {
        // Refuse to overwrite in-progress turn — a forgotten end_turn would
        // otherwise orphan the previous turn (still InProgress on disk but
        // unreferenced by current_turn_id).
        if let Some(existing) = self.store.current_turn_id() {
            warn!(
                ?existing,
                "try_start_turn called while another turn is in progress; refusing to overwrite"
            );
            return None;
        }
        let id = self.store.start_turn(trigger.clone());
        let event = TurnEvent::TurnStarted {
            turn_id: id.clone(),
            started_at: Utc::now(),
            trigger,
        };
        match logger.persist(&event) {
            Ok(()) => {
                self.refresh_view();
                Some(id)
            }
            Err(e) => {
                warn!(error = %e, "TurnStarted persist failed; rolling back");
                self.store.rollback_last_turn(&id);
                None
            }
        }
    }

    pub fn try_append_step(
        &mut self,
        step: TurnStep,
        logger: &dyn TurnEventLogger,
    ) -> Result<u32, TurnTrackerError> {
        let turn_id = self
            .store
            .current_turn_id()
            .cloned()
            .ok_or(TurnTrackerError::NoCurrentTurn)?;
        // Snapshot prior last_step_at so persist failure rolls back the
        // timestamp — otherwise microcompact's idle gauge stays biased
        // toward "recent" until the next successful append.
        let prev_last_step_at = self
            .store
            .turns()
            .iter()
            .find(|t| t.id == turn_id)
            .and_then(|t| t.last_step_at);
        let step_index = self.store.append_step(step.clone())?;
        let event = TurnEvent::StepAppended {
            turn_id: turn_id.clone(),
            step_index,
            step,
            appended_at: Some(Utc::now()),
        };
        match logger.persist(&event) {
            Ok(()) => {
                self.refresh_view();
                Ok(step_index)
            }
            Err(e) => {
                self.store
                    .rollback_last_step_with_timestamp(&turn_id, prev_last_step_at);
                Err(TurnTrackerError::PersistFailed(e))
            }
        }
    }

    pub fn end_turn(
        &mut self,
        outcome: TurnOutcome,
        logger: &dyn TurnEventLogger,
    ) -> Result<(), TurnTrackerError> {
        let turn_id = self
            .store
            .current_turn_id()
            .cloned()
            .ok_or(TurnTrackerError::NoCurrentTurn)?;
        // Pre-validate before persist: if we wrote TurnEnded first and then
        // end_current_turn failed, on-disk would show closed while memory
        // showed InProgress — split-brain unrecoverable by fold_events.
        let still_in_progress = self
            .store
            .turns()
            .iter()
            .find(|t| t.id == turn_id)
            .is_some_and(|t| matches!(t.outcome, TurnOutcome::InProgress));
        if !still_in_progress {
            return Err(TurnTrackerError::from(
                crate::turn_store::TurnStoreError::CurrentTurnFinished,
            ));
        }
        let event = TurnEvent::TurnEnded {
            turn_id: turn_id.clone(),
            outcome: outcome.clone(),
        };
        if let Err(e) = logger.persist(&event) {
            return Err(TurnTrackerError::PersistFailed(e));
        }
        self.store.end_current_turn(outcome)?;
        self.current_tool_batch_step = None;
        self.refresh_view();
        Ok(())
    }

    pub fn clear(&mut self, logger: &dyn TurnEventLogger) {
        // Single-event atomic (cancel + wipe). Two-event sequence left
        // zombie Cancelled turns on partial persist failure.
        let cancel_in_progress = self.store.current_turn_id().cloned();
        let event = TurnEvent::Cleared {
            at: Utc::now(),
            cancel_in_progress,
        };
        if let Err(e) = logger.persist(&event) {
            warn!(error = %e, "Cleared event persist failed; in-memory state retained");
            return;
        }
        self.store.clear();
        self.current_tool_batch_step = None;
        self.refresh_view();
    }

    pub fn rewind(&mut self, keep: usize, logger: &dyn TurnEventLogger) {
        let cancel_in_progress = self.store.current_turn_id().cloned();
        let event = TurnEvent::Rewound {
            at: Utc::now(),
            keep: keep as u32,
            cancel_in_progress: cancel_in_progress.clone(),
        };
        if let Err(e) = logger.persist(&event) {
            warn!(keep, error = %e, "Rewound event persist failed; in-memory state retained");
            return;
        }
        if cancel_in_progress.is_some() {
            let _ = self.store.end_current_turn(TurnOutcome::Cancelled {
                cause: loopal_turn::CancelledCause::ParentTurnAborted,
            });
        }
        self.store.truncate_turns(keep);
        self.current_tool_batch_step = None;
        self.refresh_view();
    }
}
