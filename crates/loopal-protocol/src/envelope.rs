use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::user_content::UserContent;

/// Origin of a message in the three-plane architecture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageSource {
    /// Message from a human user.
    Human,
    /// Message from a named agent.
    Agent(String),
    /// Message delivered through a pub/sub channel.
    Channel { channel: String, from: String },
    /// Message injected by the cron scheduler.
    Scheduled,
}

impl MessageSource {
    /// Short label for display and observation events.
    pub fn label(&self) -> String {
        match self {
            Self::Human => "human".to_string(),
            Self::Agent(name) => name.clone(),
            Self::Channel { from, .. } => from.clone(),
            Self::Scheduled => "scheduled".to_string(),
        }
    }
}

/// A routable message envelope.
///
/// Every inter-agent or human→agent message is wrapped in an `Envelope`.
/// The `MessageRouter` routes envelopes to the target's mailbox and
/// simultaneously emits a `MessageRouted` observation event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    /// Unique message ID for tracing and deduplication.
    pub id: Uuid,
    /// Who sent this message.
    pub source: MessageSource,
    /// Target agent name (e.g. "main", "researcher").
    pub target: String,
    /// Message content (text + optional images).
    pub content: UserContent,
    /// UTC timestamp when the envelope was created.
    pub timestamp: DateTime<Utc>,
}

impl Envelope {
    /// Create a new envelope with auto-generated ID and current timestamp.
    ///
    /// Accepts `String`, `&str`, or `UserContent` as content thanks to
    /// `Into<UserContent>` (backward-compatible with text-only callers).
    pub fn new(
        source: MessageSource,
        target: impl Into<String>,
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
}
