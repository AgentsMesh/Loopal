//! Hub-side pending permission/question state and lifecycle.
//!
//! - `handle.rs` — agent IPC entry: writes pending + emits broadcast event
//! - `resolve.rs` — UI response entry: removes pending + responds agent
//! - `cleanup.rs` — agent finish entry: drops stranded pending + emits Resolved

mod cleanup;
mod handle;
mod resolve;
mod types;

pub use handle::{handle_agent_permission, handle_agent_question};
pub use resolve::{resolve_permission, resolve_question};
pub use types::{PendingPermissionInfo, PendingQuestionInfo};

pub(crate) use cleanup::cleanup_pending_for_agent;
