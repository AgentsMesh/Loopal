use serde::{Deserialize, Serialize};

/// Origin of a `Message` — audit metadata indicating how the message
/// entered the conversation.
///
/// This is a lossy projection of the protocol-level `MessageSource`
/// (which lives in `loopal-protocol` and carries `QualifiedAddress`).
/// Keeping the projection here avoids a circular dep
/// (`loopal-message` would otherwise depend on `loopal-protocol`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MessageOrigin {
    /// Real human typing into the prompt.
    Human,
    /// Scheduled trigger (cron / timer).
    Scheduled,
    /// Routed from another agent. `label` is the address rendered for audit only.
    Agent { label: String },
    /// Routed via a named channel.
    Channel { name: String, from: String },
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
    /// Forward-compatible fallback for unrecognised system labels.
    Other { label: String },
}

impl MessageOrigin {
    /// True when the envelope marks a fresh task boundary
    /// (human input or external scheduler / cross-agent message).
    /// LoopDetector uses this to decide whether to reset signatures.
    pub fn is_task_boundary(&self) -> bool {
        matches!(
            self,
            Self::Human | Self::Scheduled | Self::Agent { .. } | Self::Channel { .. }
        )
    }

    /// True iff this origin represents a real human typing into the prompt
    /// (NOT scheduled, system-injected, or relayed from another agent).
    pub fn is_human_input(&self) -> bool {
        matches!(self, Self::Human)
    }
}
