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
    /// Wall-clock time of the most recent `append_step`. Ephemeral —
    /// not serialized. Microcompact uses this (falling back to
    /// `started_at` after replay) to gauge real idle duration.
    #[serde(skip, default)]
    pub last_step_at: Option<DateTime<Utc>>,
}

impl Turn {
    pub fn new(trigger: TurnTrigger) -> Self {
        Self {
            id: TurnId::new(),
            started_at: Utc::now(),
            trigger,
            body: TurnBody::default(),
            outcome: TurnOutcome::InProgress,
            last_step_at: None,
        }
    }

    /// Construct a single-turn for one-shot LLM callers (classifier, hooks,
    /// compaction LLM): wraps a user prompt as the trigger with no steps.
    /// Provider builds it directly via turn_projection / Anthropic
    /// build_messages_json_from_turns.
    pub fn single_user_prompt(prompt: impl Into<String>) -> Self {
        Self::new(TurnTrigger::UserInput {
            envelope_id: String::new(),
            content: prompt.into(),
            images: Vec::new(),
        })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TurnBody {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<TurnStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TurnTrigger {
    /// Real human typing into the prompt. `images` carries any base64
    /// attachments — empty for text-only input (serde default keeps old
    /// turns.jsonl readable).
    UserInput {
        envelope_id: String,
        content: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        images: Vec<loopal_tool_invocation::ToolImageBlock>,
    },
    /// Scheduled cron / timer envelope.
    Cron {
        envelope_id: String,
        content: String,
    },
    /// Routed from another agent. `from` is the address rendered as audit label
    /// (loses structural info but provider/projection just need to show it).
    Agent {
        envelope_id: String,
        from: String,
        content: String,
    },
    /// Routed via a named channel.
    Channel {
        envelope_id: String,
        channel: String,
        from: String,
        content: String,
    },
    /// `goal_continuation_check` injected continuation prompt.
    GoalContinuation {
        envelope_id: String,
        content: String,
    },
    /// Async background hook fired and queued an envelope. `hook_kind` is the
    /// short label that identifies which hook ran (e.g. `stop_feedback`,
    /// `goal_continuation`).
    BackgroundHook {
        envelope_id: String,
        hook_kind: String,
        content: String,
    },
    /// Synthetic trigger when resuming a session from disk — no real envelope.
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
