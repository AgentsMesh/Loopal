//! Message log entries for the Observation Plane.
//!
//! Captures inter-agent communication for visibility for consumers.
//! Used both per-agent and in the global message feed.

use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use loopal_protocol::{MessageSource, QualifiedAddress};

/// Single entry in the message log (observation plane).
#[derive(Debug, Clone)]
pub struct MessageLogEntry {
    pub source: String,
    pub target: String,
    pub content_preview: String,
    pub timestamp: DateTime<Utc>,
}

impl MessageLogEntry {
    pub fn new(
        source: impl Into<String>,
        target: impl Into<String>,
        content_preview: impl Into<String>,
    ) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
            content_preview: content_preview.into(),
            timestamp: Utc::now(),
        }
    }
}

/// Bounded message feed with a maximum capacity.
///
/// When the feed is full, the oldest entry is dropped on insertion.
#[derive(Debug)]
pub struct MessageFeed {
    entries: VecDeque<MessageLogEntry>,
    max_capacity: usize,
}

impl MessageFeed {
    pub fn new(max_capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_capacity.min(256)),
            max_capacity,
        }
    }

    /// Record a new entry, evicting the oldest if at capacity.
    pub fn record(&mut self, entry: MessageLogEntry) {
        if self.entries.len() >= self.max_capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// Iterate over entries from oldest to newest.
    pub fn iter(&self) -> impl Iterator<Item = &MessageLogEntry> {
        self.entries.iter()
    }

    /// Number of entries currently stored.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the feed is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the N most recent entries (newest last).
    pub fn recent(&self, n: usize) -> impl Iterator<Item = &MessageLogEntry> {
        let skip = self.entries.len().saturating_sub(n);
        self.entries.iter().skip(skip)
    }
}

/// Record a MessageRouted event to the global feed and per-agent logs.
///
/// Per-agent attribution applies only when the source/target reference a
/// **local** agent (the message log lives inside this hub's session state).
pub(crate) fn record_message_routed(
    state: &mut crate::state::SessionState,
    source: &MessageSource,
    target: &QualifiedAddress,
    preview: &str,
) {
    let entry = MessageLogEntry::new(source.label(), target.to_string(), preview);
    state.message_feed.record(entry.clone());

    // Source-side attribution: only Agent / Channel sources name a local agent.
    let source_local = match source {
        MessageSource::Agent(addr) | MessageSource::Channel { from: addr, .. } => {
            if addr.is_local() {
                Some(&addr.agent)
            } else {
                None
            }
        }
        _ => None,
    };
    if let Some(name) = source_local
        && let Some(agent) = state.agents.get_mut(name)
    {
        agent.message_log.push(entry.clone());
    }

    // Target-side attribution: same local-only rule.
    if target.is_local()
        && let Some(agent) = state.agents.get_mut(&target.agent)
    {
        agent.message_log.push(entry);
    }
}
