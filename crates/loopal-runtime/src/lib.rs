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

/// Build initial context budget from model info + settings cap.
///
/// Single entry point for all bootstrap sites — avoids hardcoding context window.
pub fn build_initial_budget(
    model: &str,
    context_tokens_cap: u32,
    system_prompt: &str,
    tool_tokens: u32,
) -> loopal_context::ContextBudget {
    use agent_loop::model_config::ModelConfig;
    let mc = ModelConfig::from_model(
        model,
        loopal_provider_api::ThinkingConfig::Auto,
        context_tokens_cap,
    );
    mc.build_budget(system_prompt, tool_tokens)
}

// Re-export structured output types from loopal-error for consumers.
pub use loopal_error::{AgentOutput, TerminateReason};
// Re-export frontend traits and agent input for external consumers.
pub use agent_input::AgentInput;
pub use frontend::traits::{AgentFrontend, EventEmitter};
