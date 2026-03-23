//! Entry types for the JSONL event log.
//!
//! Every line in the JSONL file is a `TaggedEntry`, discriminated by `_type`:
//! - `message` — a conversation message
//! - `marker`  — a control event (Clear / CompactTo / RewindTo)

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
    /// Keep only the last `keep_last` messages during replay.
    CompactTo { keep_last: usize, timestamp: String },
    /// Discard the message with `message_id` and everything after it.
    RewindTo {
        message_id: String,
        timestamp: String,
    },
}
