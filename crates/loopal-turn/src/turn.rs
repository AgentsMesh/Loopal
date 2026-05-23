use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::step::TurnStep;

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TurnId(pub String);

impl TurnId {
    pub fn new() -> Self {
        Self(format!("t-{}", Uuid::new_v4()))
    }
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TurnId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub id: TurnId,
    pub started_at: DateTime<Utc>,
    pub trigger: TurnTrigger,
    pub body: TurnBody,
    pub outcome: TurnOutcome,
}

impl Turn {
    pub fn new(trigger: TurnTrigger) -> Self {
        Self {
            id: TurnId::new(),
            started_at: Utc::now(),
            trigger,
            body: TurnBody::default(),
            outcome: TurnOutcome::InProgress,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TurnBody {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<TurnStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TurnTrigger {
    UserInput {
        envelope_id: String,
        content: String,
    },
    Cron {
        task_id: String,
        prompt: String,
    },
    GoalContinuation {
        goal_id: String,
    },
    BackgroundHook {
        hook_id: String,
        payload: serde_json::Value,
    },
    Resume,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TurnOutcome {
    InProgress,
    Complete,
    Idle { wake_at: DateTime<Utc> },
    Error { message: String },
    Cancelled { cause: CancelledCause },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CancelledCause {
    UserInterrupt,
    CrashRecovery,
    ParentTurnAborted,
}
