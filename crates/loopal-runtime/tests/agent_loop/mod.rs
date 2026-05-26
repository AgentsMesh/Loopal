use std::sync::Arc;

use loopal_config::Settings;
use loopal_context::ContextBudget;
use loopal_kernel::Kernel;
use loopal_protocol::AgentEvent;
use loopal_protocol::ControlCommand;
use loopal_protocol::Envelope;
use loopal_runtime::agent_loop::{AgentLoopRunner, cancel::TurnCancel};
use loopal_runtime::frontend::{
    DenyAllHandler, ManualPermissionHandler, UnsupportedQuestionHandler,
};
use loopal_runtime::{
    AgentConfig, AgentDeps, AgentLoopParamsBuilder, InterruptHandle, UnifiedFrontend,
};
use loopal_test_support::TestFixture;
use loopal_tool_api::PermissionMode;
use tokio::sync::mpsc;

/// Create a no-op TurnCancel for tests (never cancelled).
pub fn make_cancel() -> TurnCancel {
    TurnCancel::new(
        Default::default(),
        Arc::new(tokio::sync::watch::channel(0u64).0),
    )
}

/// Create a fresh TurnContext (turn_id=0) for tests that drive
/// `execute_tools` / `intercept_special_tools` directly.
pub fn make_turn_ctx() -> loopal_runtime::agent_loop::TurnContext {
    loopal_runtime::agent_loop::TurnContext::new(0, make_cancel())
}

/// Wrap an async block in `scope_turn(1, ...)` so in-turn emit sites
/// (`emit_in_turn`) can run inside the capability scope without panicking.
/// Use whenever a test directly calls runner methods that internally use
/// `emit_in_turn` (i.e. anything reachable from `execute_turn_body`).
pub async fn in_turn<F, R>(f: F) -> R
where
    F: std::future::Future<Output = R>,
{
    loopal_protocol::event_id::scope_turn(1, f).await
}

pub fn make_test_budget() -> ContextBudget {
    ContextBudget {
        context_window: 200_000,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: 16_384,
        safety_margin: 10_000,
        message_budget: 173_616,
        max_output_tokens: 64_000,
    }
}

mod ask_user_schema_err_test;
mod auto_continue_edge_test;
mod auto_continue_test;
mod compact_bare_summary_e2e_test;
mod compact_force_e2e_test;
mod compact_hooks_e2e_test;
mod compact_instructions_e2e_test;
mod compact_phases_e2e_test;
mod compaction_run_e2e_test;
mod cron_e2e_test;
mod degeneration_e2e_test;
mod drain_pending_test;
mod e2e_event_waiters;
mod goal_e2e_test;
mod goal_kickoff_edge_test;
mod goal_kickoff_runner_test;
mod goal_kickoff_test;
mod idle_e2e_test;
mod inbox_event_test;
mod input_edge_test;
mod input_emit_fail_edge_test;
mod input_image_test;
mod input_mcp_test;
mod input_resources_test;
mod input_scheduled_test;
mod input_test;
mod integration_test;
mod llm_test;
mod llm_truncation_test;
mod microcompact_e2e_test;
pub mod mock_provider;
mod rehydrate_e2e_test;
pub use mock_provider::make_runner_with_mock_provider;
mod cancel_test;
mod context_budget_test;
mod dispatch_test;
mod model_routing_test;
mod params_builder_test;
mod permission_test_ext;
mod plan_mode_filter_test;
mod plan_mode_test;
mod preflight_test;
mod record_message_test;
mod recovery_invariant_test;
mod resume_invariant_test;
mod resume_session_hook_test;
mod retry_cancel_test;
mod run_test;
mod stream_truncation_edge_test;
mod stream_truncation_test;
mod suspend_cron_e2e_test;
mod suspend_e2e_test;
mod thinking_continue_test;
mod tools_test;
pub mod try_recover_helpers;
mod try_recover_test;
mod turn_completion_edge_test;
mod turn_completion_test;
mod turn_test;

/// Minimal runner with no provider — for testing pure AgentLoopRunner methods.
pub fn make_runner() -> (AgentLoopRunner, mpsc::Receiver<AgentEvent>) {
    let fixture = TestFixture::new();
    let (event_tx, event_rx) = mpsc::channel(16);
    let (_mbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(16);
    let (_ctrl_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let frontend = Arc::new(UnifiedFrontend::new(
        None,
        event_tx,
        mailbox_rx,
        control_rx,
        None,
        Box::new(DenyAllHandler),
        Box::new(UnsupportedQuestionHandler),
    ));
    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
    let params = AgentLoopParamsBuilder::new(
        AgentConfig::default(),
        AgentDeps {
            kernel,
            frontend,
            session_manager: fixture.session_manager(),
            decision_context: loopal_runtime::frontend::DecisionContext::with_cwd("/tmp/test"),
        },
        fixture.test_session("test-minimal"),
        make_test_budget(),
        InterruptHandle::new(),
    )
    .build();
    let mut runner = AgentLoopRunner::new(params);
    runner.start_turn_record(loopal_turn::TurnTrigger::Resume);
    (runner, event_rx)
}

/// Runner with all channels exposed — for testing permission and input flows.
pub fn make_runner_with_channels() -> (
    AgentLoopRunner,
    mpsc::Receiver<AgentEvent>,
    mpsc::Sender<Envelope>,
    mpsc::Sender<ControlCommand>,
    mpsc::Sender<bool>,
) {
    let fixture = TestFixture::new();
    let (event_tx, event_rx) = mpsc::channel(16);
    let (mbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(16);
    let (ctrl_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, permission_rx) = mpsc::channel::<bool>(16);
    let frontend = Arc::new(UnifiedFrontend::new(
        None,
        event_tx.clone(),
        mailbox_rx,
        control_rx,
        None,
        Box::new(ManualPermissionHandler::new(event_tx, permission_rx)),
        Box::new(UnsupportedQuestionHandler),
    ));
    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
    let params = AgentLoopParamsBuilder::new(
        AgentConfig {
            permission_mode: PermissionMode::AskAnyWrite,
            ..Default::default()
        },
        AgentDeps {
            kernel,
            frontend,
            session_manager: fixture.session_manager(),
            decision_context: loopal_runtime::frontend::DecisionContext::with_cwd("/tmp/test"),
        },
        fixture.test_session("test-channels"),
        make_test_budget(),
        InterruptHandle::new(),
    )
    .build();
    let mut runner = AgentLoopRunner::new(params);
    runner.start_turn_record(loopal_turn::TurnTrigger::Resume);
    (runner, event_rx, mbox_tx, ctrl_tx, perm_tx)
}
