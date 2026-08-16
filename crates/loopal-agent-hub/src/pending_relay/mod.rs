//! Hub-side pending permission/question state and lifecycle.
//!
//! - `handle.rs` — agent IPC entry: writes pending + emits broadcast event
//! - `permission_resolution.rs` — permission response entry: removes pending + responds agent
//! - `cleanup.rs` — agent finish entry: drops stranded pending + emits Resolved

mod cleanup;
mod completion;
mod handle;
mod permission_audit;
mod permission_delivery;
mod permission_fast_path;
mod permission_request;
mod permission_resolution;
mod plan;
mod question;
mod remote;
mod resolve;
mod types;

pub(crate) use handle::handle_agent_permission;
pub(crate) use permission_resolution::resolve_permission;
pub use plan::{handle_agent_plan_approval, resolve_plan_approval};
pub use question::handle_agent_question;
pub use resolve::resolve_question;
pub(crate) use resolve::resolve_remote_question;
#[cfg(test)]
pub(crate) use types::InteractionAudience;
#[cfg(test)]
#[path = "resolve_tests.rs"]
mod resolve_tests;
pub(crate) use types::OriginRemoteQuestionCancel;
pub use types::{
    PendingPermissionInfo, PendingPlanApprovalInfo, PendingQuestionInfo, PendingRemoteQuestionInfo,
};

pub(crate) use cleanup::{
    cancel_pending_for_agent_connection, cancel_pending_request, cleanup_pending_for_agent,
    cleanup_pending_for_agent_connection, cleanup_pending_for_uplink, cleanup_without_capable_ui,
};
pub(crate) use completion::deliver_terminal_event;
