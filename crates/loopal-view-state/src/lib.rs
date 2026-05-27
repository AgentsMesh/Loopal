pub mod conversation;
pub mod delta;
mod mutators;
pub mod reducer;
pub mod state;
pub mod view_proto;

pub use conversation::{
    AgentConversation, InboxOrigin, PendingPermission, PendingQuestion, PermissionChoice,
    SessionMessage, format_thinking_content, format_token_display, parse_thinking_content,
};
pub use delta::ViewSnapshot;
pub use loopal_tool_invocation::{
    CancelCause, FailureKind, InvocationId, InvocationState, Outcome, ProgressSnapshot,
    StaleReason, ToolInvocation, ToolResultMetadata,
};
pub use reducer::ViewStateReducer;
pub use state::{AgentView, BgTaskView, SessionViewState};
pub use view_proto::ViewSnapshotRequest;
