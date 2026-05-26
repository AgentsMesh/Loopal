use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::step::{ToolExecState, TurnStep};
use crate::turn::{TurnId, TurnOutcome, TurnTrigger};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TurnEvent {
    TurnStarted {
        turn_id: TurnId,
        started_at: DateTime<Utc>,
        trigger: TurnTrigger,
    },
    StepAppended {
        turn_id: TurnId,
        step_index: u32,
        step: TurnStep,
        // reason: persisted so fold_events can restore `Turn.last_step_at`
        // after restart — microcompact reads that to gauge idle time.
        // Without it, replay falls back to `Turn.started_at` and may
        // immediately scrub fresh tool results post-resume.
        // serde-default keeps existing turns.jsonl readable (older events
        // have no appended_at; fold_events tolerates that by leaving
        // last_step_at = None, which still falls back to started_at).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        appended_at: Option<DateTime<Utc>>,
    },
    StepUpdated {
        turn_id: TurnId,
        step_index: u32,
        item_index: u32,
        new_state: ToolExecState,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        updated_at: Option<DateTime<Utc>>,
    },
    TurnEnded {
        turn_id: TurnId,
        outcome: TurnOutcome,
    },
    /// User invoked `/clear`: wipe all prior turns. Replay folds this by
    /// discarding everything before it, so a session resumed from a log that
    /// contains a `Cleared` event starts from a clean state.
    /// `cancel_in_progress` lets cancel + wipe land atomically in one event.
    Cleared {
        at: DateTime<Utc>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cancel_in_progress: Option<TurnId>,
    },
    /// User invoked `/rewind`: keep only the first `keep` turns. Replay
    /// folds by truncating the in-memory turn list at this point.
    Rewound {
        at: DateTime<Utc>,
        keep: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cancel_in_progress: Option<TurnId>,
    },
}
