//! Entry types for the JSONL event log.
//!
//! Every line in the JSONL file is a `TaggedEntry`, discriminated by `_type`:
//! - `message` — a conversation message
//! - `marker`  — a control event (Clear / CompactBoundary / RewindTo)

use loopal_message::Message;
use serde::{Deserialize, Serialize};

/// A single line in the JSONL event log.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "_type", rename_all = "snake_case")]
pub enum TaggedEntry {
    Message(Message),
    Marker(Marker),
}

/// Control markers that modify replay semantics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Marker {
    /// Discard all preceding entries during replay.
    Clear { timestamp: String },
    /// Anchor point produced by a compaction: replay drops every message
    /// before the message whose `id == summary_msg_id`, keeping it and
    /// everything after.
    CompactBoundary {
        summary_msg_id: String,
        timestamp: String,
    },
    /// Discard the message with `message_id` and everything after it.
    RewindTo {
        message_id: String,
        timestamp: String,
    },
}
