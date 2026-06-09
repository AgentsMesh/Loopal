pub mod cancel;
mod cold_start_emit;
mod compact_rehydrate;
mod compaction;
mod compaction_run;
mod context_pipeline;
pub mod continuation_gate;
pub mod degeneration_detector;
mod degeneration_feedback;
pub mod diff_tracker;
pub mod env_context;
mod finished_guard;
mod fork_snapshot;
mod goal_consistency;
mod goal_continuation;
mod goal_control;
pub mod governance;
mod handle_request_idle;
mod hub_health;
mod ingest;
mod input;
mod input_control;
mod input_control_config;
mod input_mcp;
mod input_resources;
mod llm;
mod llm_chunk;
mod llm_params;
mod llm_record;
pub(crate) mod llm_result;
mod llm_retry;
pub mod loop_detector;
pub(crate) mod model_config;
mod params;
mod params_builder;
mod permission;
mod pipeline_setup;
pub mod question_format;
pub mod question_parse;
mod resume_session;
pub mod rewind;
mod run;
pub use run::CONTEXT_OVERFLOW_BANNER;
mod runner;
mod runner_transition;
/// Sandbox path pre-check utilities for the tools_check phase.
/// Public for integration testing; runtime consumers should use tools_check directly.
pub mod sandbox_precheck;
mod stop_feedback;
pub(crate) mod streaming_tool_exec;
pub(crate) mod token_accumulator;
mod tool_collect;
pub(crate) mod tool_exec;
mod tool_progress;
mod tool_watchdog;
mod tools;
mod tools_ask_user;
mod tools_check;
mod tools_check_one;
mod tools_finalize;
mod tools_inject;
mod tools_intercept;
mod tools_phase;
pub(crate) mod tools_plan;
mod tools_plan_exit;
mod tools_resolve;
mod turn_cancel_finalize;
pub mod turn_context;
mod turn_exec;
pub mod turn_history;
pub(crate) mod turn_metrics;
mod turn_observer_dispatch;
mod turn_record;
mod turn_recover;
mod turn_response;
mod turn_state;
mod turn_telemetry;
mod turn_tool_phase;
mod turn_trigger_map;

use loopal_error::{AgentOutput, Result};

pub use input::WaitResult;
pub use params::{
    AgentConfig, AgentDeps, AgentLoopParams, InterruptHandle, LifecycleMode, PlanModeState,
};
pub use params_builder::AgentLoopParamsBuilder;
pub use runner::AgentLoopRunner;
pub use streaming_tool_exec::StreamingToolHandle;
pub use turn_context::TurnContext;

use finished_guard::FinishedGuard;

pub async fn agent_loop(params: AgentLoopParams) -> Result<AgentOutput> {
    let mut guard = FinishedGuard::new(params.deps.frontend.clone());
    let governance = governance::build_governance(&params.harness);
    let hooks = governance::build_hooks(params.deps.frontend.clone());
    let config_snapshots = pipeline_setup::build_config_snapshots(&params.session.cwd);
    let mut runner = AgentLoopRunner::new(params);
    runner.governance = governance;
    runner.hooks = hooks;
    runner.config_snapshots = config_snapshots;
    let result = runner.run().await;
    guard.disarm();
    result
}

pub(crate) struct TurnOutput {
    pub output: String,
}
