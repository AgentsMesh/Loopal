//! Replay engine: fold a sequence of `TaggedEntry` into `Vec<Message>`.
//!
//! Markers alter the accumulated message list:
//! - `Clear` discards everything before it.
//! - `CompactTo { keep_last }` trims to the most recent N messages.
//! - `RewindTo { message_id }` discards the target message and everything after it.

use loopal_message::Message;

use crate::entry::{Marker, TaggedEntry};

/// Replay storage entries into a final message list.
///
/// Entries are processed in order. Markers modify the accumulated
/// result as they are encountered.
pub fn replay(entries: Vec<TaggedEntry>) -> Vec<Message> {
    let mut messages: Vec<Message> = Vec::new();

    for entry in entries {
        match entry {
            TaggedEntry::Message(msg) => messages.push(msg),
            TaggedEntry::Marker(marker) => apply_marker(&mut messages, &marker),
        }
    }

    messages
}

fn apply_marker(messages: &mut Vec<Message>, marker: &Marker) {
    match marker {
        Marker::Clear { .. } => messages.clear(),
        Marker::CompactTo { keep_last, .. } => {
            if messages.len() > *keep_last {
                let drain_end = messages.len() - keep_last;
                messages.drain(..drain_end);
            }
        }
        Marker::RewindTo { message_id, .. } => {
            if let Some(pos) = messages
                .iter()
                .position(|m| m.id.as_deref() == Some(message_id.as_str()))
            {
                messages.truncate(pos);
            }
        }
    }
}
