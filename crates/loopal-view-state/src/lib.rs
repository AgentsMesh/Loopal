pub mod conversation;
pub mod delta;
mod mutators;
pub mod reducer;
pub mod state;
pub mod view_proto;

pub use conversation::{
    AgentConversation, InboxOrigin, PendingPermission, PendingPlanApproval, PendingQuestion,
    PermissionChoice, SessionMessage, format_thinking_content, format_token_display,
    into_session_message, parse_thinking_content,
};
pub use delta::ViewSnapshot;
pub use loopal_tool_invocation::{
    CancelCause, FailureKind, InvocationId, InvocationState, Outcome, ProgressSnapshot,
    StaleReason, ToolInvocation, ToolResultMetadata,
};
pub use reducer::ViewStateReducer;
pub use state::{AgentView, BgTaskView, SessionViewState};
pub use view_proto::ViewSnapshotRequest;
