pub mod agent_loop;
pub mod frontend;
pub mod mode;
pub mod permission;
pub mod session;
pub mod tool_pipeline;

pub use agent_loop::{agent_loop, AgentLoopParams};
pub use frontend::unified::UnifiedFrontend;
pub use mode::AgentMode;
pub use permission::check_permission;
pub use session::SessionManager;

// Re-export structured output types from loopal-types for consumers.
pub use loopal_types::error::{AgentOutput, TerminateReason};
