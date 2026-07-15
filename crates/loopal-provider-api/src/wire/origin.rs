use serde::{Deserialize, Serialize};

// reason: Audit metadata for a wire-format Message — describes how the
// message entered the conversation. Lossy projection of the protocol-level
// `MessageSource` (which carries `QualifiedAddress`); kept here in the wire
// schema crate so neither domain (loopal-turn) nor protocol need to know
// about it. Consumed by middleware / UI / forensic replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MessageOrigin {
    /// Real human typing into the prompt.
    Human,
    HumanSkill {
        name: String,
        user_args: String,
    },
    /// Scheduled trigger (cron / timer).
    Scheduled,
    /// Routed from another agent. `label` is the address rendered for audit only.
    Agent {
        label: String,
    },
    /// Routed via a named channel.
    Channel {
        name: String,
        from: String,
    },
    /// Goal-kickoff continuation envelope produced by `goal_continuation_check`.
    GoalContinuation,
    /// LoopDetector abort compensation (synthetic tool_result stubs).
    GovernanceCompensation,
    /// LoopDetector abort feedback to model (system note).
    GovernanceFeedback,
    /// Stop-hook `additional_context` fed back into the conversation.
    StopFeedback,
    /// Context-refresh middleware reminder.
    ConfigRefresh,
    /// Compaction summary produced by smart_compact middleware.
    CompactionSummary,
    /// Post-compaction rehydrate `Read` tool_use/tool_result pair injected
    /// by `compact_rehydrate` to restore top-N touched files.
    CompactionRehydrate,
    /// Forward-compatible fallback for unrecognised system labels.
    Other {
        label: String,
    },
}

impl MessageOrigin {
    /// True when the envelope marks a fresh task boundary
    /// (human input or external scheduler / cross-agent message).
    /// LoopDetector uses this to decide whether to reset signatures.
    pub fn is_task_boundary(&self) -> bool {
        matches!(
            self,
            Self::Human
                | Self::HumanSkill { .. }
                | Self::Scheduled
                | Self::Agent { .. }
                | Self::Channel { .. }
        )
    }

    /// True iff this origin represents a real human typing into the prompt
    /// (NOT scheduled, system-injected, or relayed from another agent).
    pub fn is_human_input(&self) -> bool {
        matches!(self, Self::Human | Self::HumanSkill { .. })
    }

    /// True iff this origin is a compaction artifact (summary, ack, or
    /// rehydrate Read pair). Used by forensic replay + signature reset
    /// hooks that treat compaction-injected messages as a single unit.
    pub fn is_compaction_artifact(&self) -> bool {
        matches!(self, Self::CompactionSummary | Self::CompactionRehydrate)
    }
}
