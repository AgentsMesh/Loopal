//! Replay engine: fold a sequence of `TaggedEntry` into `Vec<Message>`.
//!
//! Markers alter the accumulated message list:
//! - `Clear` discards everything before it.
//! - `CompactBoundary { summary_msg_id }` drops every message before the one
//!   whose `id == summary_msg_id` (keeping it and everything after).
//! - `RewindTo { message_id }` discards the target message and everything after it.

use loopal_provider_api::Message;

use crate::entry::{Marker, TaggedEntry};

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
        Marker::CompactBoundary { summary_msg_id, .. } => {
            if let Some(pos) = messages
                .iter()
                .position(|m| m.id.as_deref() == Some(summary_msg_id.as_str()))
            {
                messages.drain(..pos);
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
