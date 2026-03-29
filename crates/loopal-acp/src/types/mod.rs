//! ACP protocol types — re-exports from `agent-client-protocol-schema`
//! plus local helpers for Loopal-specific conversions.

pub(crate) mod convert;

pub use agent_client_protocol_schema::{
    // Identifiers
    AgentCapabilities,
    // Content
    ContentBlock,
    ContentChunk,
    Implementation,
    // Session
    NewSessionRequest,
    NewSessionResponse,
    // Permission
    PermissionOption,
    PermissionOptionId,
    PermissionOptionKind,
    PromptRequest,
    PromptResponse,
    ProtocolVersion,
    RequestPermissionOutcome,
    RequestPermissionRequest,
    RequestPermissionResponse,
    SelectedPermissionOutcome,
    SessionId,
    // Session update
    SessionNotification,
    SessionUpdate,
    // Stop reason
    StopReason,
    TextContent,
    // Tool call
    ToolCall,
    ToolCallId,
    ToolCallStatus,
    ToolCallUpdate,
    ToolCallUpdateFields,
    ToolKind,
};

pub use convert::{
    make_init_response, make_new_session_response, make_prompt_response, make_session_notification,
    text_content_block,
};
