pub mod address;
pub mod agent_completion;
pub mod agent_state;
pub mod agent_state_snapshot;
pub mod bg_task;
pub mod command;
pub mod control;
pub mod cron_snapshot;
pub mod envelope;
pub mod event;
pub mod event_id;
pub mod event_payload;
mod event_snat;
pub mod event_summary;
pub mod interaction;
pub mod interrupt;
pub mod mcp_ipc;
pub mod mcp_snapshot;
pub mod naming;
pub mod projected;
pub mod question;
pub mod secret_ipc;
pub mod task_snapshot;
pub mod thread_goal;
pub mod ui_capabilities;
pub mod user_content;

pub const META_HUB_TOKEN_ENV: &str = "LOOPAL_META_HUB_TOKEN";

pub use address::QualifiedAddress;
pub use agent_completion::{
    AgentCompletion, NO_AGENT_OUTPUT, PROTOCOL_ERROR_REASON, TRANSPORT_ERROR_REASON,
    WAIT_AGENT_TYPED_RESPONSE_V1, WaitAgentResponse, WaitAgentStatus,
};
pub use agent_state::{AgentStatus, ObservableAgentState};
pub use agent_state_snapshot::AgentStateSnapshot;
pub use bg_task::{BgTaskDetail, BgTaskSnapshot, BgTaskStatus};
pub use command::AgentMode;
pub use control::{
    CONTROL_RPC_COMPLETION_GRACE, ControlCommand, ControlDisposition,
    DEFAULT_CONTROL_APPLICATION_TIMEOUT, DEFAULT_CONTROL_RPC_TIMEOUT,
};
pub use cron_snapshot::CronJobSnapshot;
pub use envelope::{Envelope, MessageSource};
pub use event::AgentEvent;
pub use event_payload::AgentEventPayload;
pub use event_summary::{
    CompactPhase, CompactionSummary, ContinuationGateSummary, DegenerationSignal,
    DegenerationSummary, GateCloseReason, SubAgentSpawn, TurnSummary,
};
pub use interaction::{
    DEFAULT_INTERACTION_LIFETIME, DEFAULT_INTERACTION_RPC_TIMEOUT, INTERACTION_RPC_COMPLETION_GRACE,
};
pub use interrupt::InterruptSignal;
pub use mcp_ipc::{
    McpCallToolRequest, McpCallToolResponse, McpContentBlock, McpListToolsResponse,
    McpSnapshotResponse, McpToolEntry,
};
pub use mcp_snapshot::McpServerSnapshot;
pub use naming::ROOT_AGENT_NAME;
pub use projected::{ProjectedMessage, ProjectedToolCall, SessionHistorySnapshot};
pub use question::{Question, QuestionOption, ResolveSource, UserQuestionResponse};
pub use secret_ipc::{
    SecretCaller, SecretGetRequest, SecretGetResponse, SecretHealthRequest, SecretHealthResponse,
    SecretIpcError, SecretListNamesRequest, SecretListNamesResponse,
};
pub use task_snapshot::{TaskSnapshot, TaskSnapshotStatus};
pub use thread_goal::{GoalTransitionReason, ThreadGoal, ThreadGoalStatus};
pub use ui_capabilities::{UiCapabilities, UiCapability};
pub use user_content::{ImageAttachment, SkillInvocation, UserContent};
