//! IPC payload types for `view/*` protocol methods.
//!
//! Currently only `view/snapshot` request type. UI clients receive
//! incremental updates through the existing `agent/event` broadcast
//! and apply each event to a local `ViewClient` reducer; no separate
//! delta channel.

use serde::{Deserialize, Serialize};

/// `view/snapshot` request payload — UI asks for the current state of
/// one agent's view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewSnapshotRequest {
    pub agent: String,
}
