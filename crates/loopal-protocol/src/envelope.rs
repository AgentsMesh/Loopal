use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::address::QualifiedAddress;
use crate::user_content::UserContent;

/// Origin of a message in the three-plane architecture.
///
/// `Agent` and `Channel.from` carry a [`QualifiedAddress`] so receivers
/// see the full return path after NAT (`apply_snat`) at hub boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageSource {
    Human,
    Agent(QualifiedAddress),
    AgentResult {
        child: QualifiedAddress,
    },
    Channel {
        channel: String,
        from: QualifiedAddress,
    },
    Scheduled,
    System(String),
}

impl MessageSource {
    /// Short label for display and observation events.
    pub fn label(&self) -> String {
        match self {
            Self::Human => "human".to_string(),
            Self::Agent(addr) => addr.to_string(),
            Self::AgentResult { child } => child.to_string(),
            Self::Channel { from, .. } => from.to_string(),
            Self::Scheduled => "scheduled".to_string(),
            Self::System(kind) => format!("system:{kind}"),
        }
    }

    /// True for sources whose UI representation is rendered optimistically
    /// at the input site (e.g. human typing into the prompt) rather than
    /// via the inbox event pipeline. Subscribers that re-render based on
    /// `InboxEnqueued` must skip these to avoid duplicating the optimistic
    /// row. Centralised here so adding a new source forces a deliberate
    /// decision rather than silently inheriting one of the branches.
    pub fn is_optimistically_rendered(&self) -> bool {
        matches!(self, Self::Human)
    }

    /// True for sources that should lift `AgentStatus::Suspended` on ingest.
    /// Only `Human` input wakes the session; cron / system / agent / channel
    /// envelopes do not (they are exactly what `/suspend` is meant to silence).
    /// Centralised here so adding a new source forces a deliberate decision.
    pub fn wakes_suspended_session(&self) -> bool {
        matches!(self, Self::Human)
    }

    /// True for sources whose messages feed the LLM context but should not
    /// appear in user-visible session history (e.g. `goal_continuation` /
    /// scheduled cron prompts). Used at ingest time to set the persisted
    /// `Message.ephemeral_in_history` flag.
    pub fn is_ephemeral_in_history(&self) -> bool {
        matches!(self, Self::Scheduled | Self::System(_))
    }

    // Hot path predicate (called on every envelope received). Mirrors
    // `MessageOrigin::is_task_boundary` but works directly on the protocol
    // type to avoid the String allocations that `MessageOrigin::from(&self)`
    // incurs for Agent/Channel variants. SSOT pinned by
    // `tests/suite/task_boundary_test.rs`.
    pub fn is_task_boundary(&self) -> bool {
        matches!(
            self,
            Self::Human
                | Self::Scheduled
                | Self::Agent(_)
                | Self::AgentResult { .. }
                | Self::Channel { .. }
        )
    }

    /// SNAT — prepend a hub name into any addressable origin field.
    /// Variants without an addressable origin (Human/Scheduled/System) are no-ops.
    pub fn prepend_hub(&mut self, self_hub: &str) {
        match self {
            Self::Agent(addr) => addr.prepend_hub(self_hub.to_string()),
            Self::AgentResult { child } => child.prepend_hub(self_hub.to_string()),
            Self::Channel { from, .. } => from.prepend_hub(self_hub.to_string()),
            _ => {}
        }
    }

    /// Conditional SNAT — prepend only if the inner address is still local.
    /// Used by event aggregation where the source may already be qualified
    /// (e.g. an envelope routed from another hub).
    pub fn prepend_hub_if_local(&mut self, self_hub: &str) {
        match self {
            Self::Agent(addr) => addr.prepend_hub_if_local(self_hub.to_string()),
            Self::AgentResult { child } => child.prepend_hub_if_local(self_hub.to_string()),
            Self::Channel { from, .. } => from.prepend_hub_if_local(self_hub.to_string()),
            _ => {}
        }
    }
}

/// A routable message envelope.
///
/// `target` is a [`QualifiedAddress`]; routing layers consume hub segments
/// via [`Envelope::apply_dnat`] as the envelope crosses boundaries inbound,
/// and stamp source hub names via [`Envelope::apply_snat`] outbound.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub id: Uuid,
    pub source: MessageSource,
    pub target: QualifiedAddress,
    pub content: UserContent,
    pub timestamp: DateTime<Utc>,
    /// Sender-supplied UI preview, distinct from `content_preview()` truncation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

impl Envelope {
    pub fn new(
        source: MessageSource,
        target: impl Into<QualifiedAddress>,
        content: impl Into<UserContent>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            source,
            target: target.into(),
            content: content.into(),
            timestamp: Utc::now(),
            summary: None,
        }
    }

    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    pub fn content_preview(&self) -> &str {
        self.content.text_preview()
    }

    /// SNAT — stamp this hub's name onto the source path. Apply once
    /// when an envelope crosses an outbound hub boundary.
    pub fn apply_snat(&mut self, self_hub: &str) {
        self.source.prepend_hub(self_hub);
    }

    /// DNAT — pop the next-hop hub from the target path. Returns the
    /// consumed hub for diagnostics, or `None` if already local.
    pub fn apply_dnat(&mut self) -> Option<String> {
        self.target.pop_front_hub()
    }
}
