pub mod conversation;
pub mod delta;
mod mutators;
pub mod reducer;
pub mod state;
pub mod view_proto;

pub use conversation::{
    AgentConversation, InboxOrigin, PendingPermission, PendingQuestion, SessionMessage,
    SessionToolCall, ToolCallStatus, format_thinking_content, format_token_display,
    parse_thinking_content,
};
pub use delta::ViewSnapshot;
pub use reducer::ViewStateReducer;
pub use state::{AgentView, BgTaskView, SessionViewState};
pub use view_proto::ViewSnapshotRequest;
