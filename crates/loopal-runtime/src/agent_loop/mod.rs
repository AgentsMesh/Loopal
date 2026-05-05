pub mod cancel;
mod compaction;
mod context_pipeline;
pub mod diff_tracker;
pub mod env_context;
mod finished_guard;
mod fork_snapshot;
mod input;
mod input_control;
mod input_mcp;
mod llm;
mod llm_params;
mod llm_record;
pub(crate) mod llm_result;
mod llm_retry;
pub mod loop_detector;
pub(crate) mod message_build;
pub(crate) mod model_config;
mod params;
mod params_builder;
mod permission;
mod pipeline_setup;
pub mod question_parse;
mod resume_session;
pub mod rewind;
mod run;
mod runner;
/// Sandbox path pre-check utilities for the tools_check phase.
/// Public for integration testing; runtime consumers should use tools_check directly.
pub mod sandbox_precheck;
mod stop_feedback;
pub(crate) mod streaming_tool_exec;
pub(crate) mod token_accumulator;
mod tool_collect;
pub(crate) mod tool_exec;
mod tool_progress;
mod tools;
mod tools_check;
mod tools_inject;
pub(crate) mod tools_plan;
mod tools_resolve;
pub mod turn_context;
mod turn_exec;
pub(crate) mod turn_metrics;
pub mod turn_observer;
mod turn_observer_dispatch;
mod turn_response;
mod turn_state;
mod turn_telemetry;
mod turn_tool_phase;

use loopal_error::{AgentOutput, Result};

pub use params::{
    AgentConfig, AgentDeps, AgentLoopParams, InterruptHandle, LifecycleMode, PlanModeState,
};
pub use params_builder::AgentLoopParamsBuilder;
pub use runner::AgentLoopRunner;

use finished_guard::FinishedGuard;

pub async fn agent_loop(params: AgentLoopParams) -> Result<AgentOutput> {
    let mut guard = FinishedGuard::new(params.deps.frontend.clone());
    let h = &params.harness;
    let observers: Vec<Box<dyn turn_observer::TurnObserver>> = vec![
        Box::new(loop_detector::LoopDetector::with_thresholds(
            h.loop_warn_threshold,
            h.loop_abort_threshold,
        )),
        Box::new(diff_tracker::DiffTracker::new(params.deps.frontend.clone())),
    ];
    let pipeline = pipeline_setup::build_context_pipeline(&params.session.cwd);
    let mut runner = AgentLoopRunner::new(params);
    runner.observers = observers;
    runner.pipeline = pipeline;
    let result = runner.run().await;
    guard.disarm();
    result
}

pub(crate) struct TurnOutput {
    pub output: String,
}
