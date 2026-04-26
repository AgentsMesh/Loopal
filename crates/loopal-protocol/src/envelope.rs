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
            Self::Channel { from, .. } => from.to_string(),
            Self::Scheduled => "scheduled".to_string(),
            Self::System(kind) => format!("system:{kind}"),
        }
    }

    /// SNAT — prepend a hub name into any addressable origin field.
    /// Variants without an addressable origin (Human/Scheduled/System) are no-ops.
    pub fn prepend_hub(&mut self, self_hub: &str) {
        match self {
            Self::Agent(addr) => addr.prepend_hub(self_hub.to_string()),
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
}

impl Envelope {
    /// Create a new envelope with auto-generated ID and current timestamp.
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
        }
    }

    /// Short preview of the text content (max ~80 chars, safe for multi-byte).
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
