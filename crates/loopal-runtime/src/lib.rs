pub mod agent_input;
pub mod agent_loop;
pub mod frontend;
pub mod mode;
pub mod permission;
pub mod projection;
pub mod session;
pub mod tool_pipeline;

pub use agent_loop::{AgentConfig, AgentDeps, AgentLoopParams, InterruptHandle, agent_loop};
pub use frontend::unified::UnifiedFrontend;
pub use mode::AgentMode;
pub use permission::check_permission;
pub use session::SessionManager;

// Re-export structured output types from loopal-error for consumers.
pub use loopal_error::{AgentOutput, TerminateReason};
// Re-export frontend traits and agent input for external consumers.
pub use agent_input::AgentInput;
pub use frontend::traits::{AgentFrontend, EventEmitter};
