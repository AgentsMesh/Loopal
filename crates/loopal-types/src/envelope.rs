use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Origin of a message in the three-plane architecture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageSource {
    /// Message from a human user (via TUI).
    Human,
    /// Message from a named agent.
    Agent(String),
    /// Message delivered through a pub/sub channel.
    Channel { channel: String, from: String },
}

impl MessageSource {
    /// Short label for display and observation events.
    pub fn label(&self) -> String {
        match self {
            Self::Human => "human".to_string(),
            Self::Agent(name) => name.clone(),
            Self::Channel { from, .. } => from.clone(),
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
    /// Message content.
    pub content: String,
    /// UTC timestamp when the envelope was created.
    pub timestamp: DateTime<Utc>,
}

impl Envelope {
    /// Create a new envelope with auto-generated ID and current timestamp.
    pub fn new(source: MessageSource, target: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            source,
            target: target.into(),
            content: content.into(),
            timestamp: Utc::now(),
        }
    }

    /// Short preview of the content (max ~80 chars, safe for multi-byte).
    pub fn content_preview(&self) -> &str {
        let s = self.content.as_str();
        if s.len() <= 80 {
            s
        } else {
            // Find the nearest char boundary at or before byte 80
            let mut end = 80;
            while end > 0 && !s.is_char_boundary(end) {
                end -= 1;
            }
            &s[..end]
        }
    }
}
