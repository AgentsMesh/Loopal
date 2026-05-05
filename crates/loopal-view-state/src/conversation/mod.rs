//! Per-agent conversation state and event-driven mutation.

mod agent_conversation;
pub(crate) mod conversation_display;
pub(crate) mod server_tool_display;
pub(crate) mod thinking_display;
pub(crate) mod tool_result_handler;
pub(crate) mod truncate;
mod types;

pub use agent_conversation::AgentConversation;
pub use server_tool_display::format_server_tool_content;
pub use thinking_display::{format_thinking_content, format_token_display, parse_thinking_content};
pub use types::{
    InboxOrigin, PendingPermission, PendingQuestion, SessionMessage, SessionToolCall,
    ToolCallStatus,
};
