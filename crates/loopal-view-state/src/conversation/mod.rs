//! Per-agent conversation state and event-driven mutation.

mod agent_conversation;
mod classifier_status;
pub(crate) mod conversation_display;
mod pending_question;
mod projected_message;
mod question_state;
pub(crate) mod server_tool_display;
pub(crate) mod thinking_display;
pub(crate) mod tool_result_handler;
pub(crate) mod truncate;
mod types;

pub use agent_conversation::AgentConversation;
pub use classifier_status::ClassifierStatus;
pub use pending_question::PendingQuestion;
pub use projected_message::into_session_message;
pub use question_state::QuestionState;
pub use server_tool_display::format_server_tool_content;
pub use thinking_display::{format_thinking_content, format_token_display, parse_thinking_content};
pub use types::{
    InboxOrigin, PendingPermission, PendingPlanApproval, PermissionChoice, SessionMessage,
};
