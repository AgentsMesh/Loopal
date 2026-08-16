pub mod agent_input;
pub mod agent_loop;
mod agent_output_guard;
pub mod fire_hooks;
pub mod frontend;
pub mod goal;
pub mod hydrate;
mod image_limits;
pub mod mode;
pub mod otel_metrics;
pub mod permission;
pub mod plan_file;
mod process_guarded_backend;
mod process_output_sanitizer;
pub mod session;
pub mod session_resume_hook;
pub mod tool_action;
mod tool_effect_secrets;
mod tool_execution_output;
mod tool_input_validation;
pub mod tool_pipeline;
pub mod tool_prepare;
mod tool_result_guard;
pub mod workflow_input;
pub mod workflow_lease;

pub use agent_loop::{
    AgentConfig, AgentDeps, AgentLoopParams, AgentLoopParamsBuilder, InterruptHandle,
    LifecycleMode, agent_loop,
};
pub use frontend::unified::UnifiedFrontend;
pub use goal::{GoalRuntimeSession, GoalSessionToolAdapter};
pub use mode::AgentMode;
pub use permission::check_permission;
pub use session::SessionManager;
pub use session_resume_hook::{SessionResumeError, SessionResumeHook};
pub use workflow_lease::WorkflowLeaseTracker;

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
pub use frontend::traits::{
    AgentFrontend, EventEmitter, PlanApproval, PlanApprovalCancellationReason,
};
