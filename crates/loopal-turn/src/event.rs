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
    },
    StepUpdated {
        turn_id: TurnId,
        step_index: u32,
        item_index: u32,
        new_state: ToolExecState,
    },
    TurnEnded {
        turn_id: TurnId,
        outcome: TurnOutcome,
    },
}
