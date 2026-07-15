//! Hub-side pending permission/question state and lifecycle.
//!
//! - `handle.rs` — agent IPC entry: writes pending + emits broadcast event
//! - `resolve.rs` — UI response entry: removes pending + responds agent
//! - `cleanup.rs` — agent finish entry: drops stranded pending + emits Resolved

mod cleanup;
mod handle;
mod plan;
mod remote;
mod resolve;
mod types;

pub use handle::{handle_agent_permission, handle_agent_question};
pub use plan::{handle_agent_plan_approval, resolve_plan_approval};
pub use resolve::{resolve_permission, resolve_question};
pub use types::{PendingPermissionInfo, PendingPlanApprovalInfo, PendingQuestionInfo};

pub(crate) use cleanup::cleanup_pending_for_agent;
