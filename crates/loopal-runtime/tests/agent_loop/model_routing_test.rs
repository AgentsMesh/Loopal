use std::collections::HashMap;
use std::sync::Arc;

use loopal_config::Settings;
use loopal_context::ContextStore;
use loopal_kernel::Kernel;
use loopal_protocol::{AgentEvent, ControlCommand, Envelope};
use loopal_provider_api::{ModelRouter, TaskType};
use loopal_runtime::agent_loop::AgentLoopRunner;
use loopal_runtime::frontend::{AutoCancelQuestionHandler, AutoDenyHandler};
use loopal_runtime::{AgentConfig, AgentDeps, AgentLoopParams, InterruptHandle, UnifiedFrontend};
use loopal_test_support::TestFixture;
use tokio::sync::mpsc;

use super::make_test_budget;

/// Build a runner with a custom summarization model in the router.
fn make_runner_with_routing(
    summarization_model: &str,
) -> (AgentLoopRunner, mpsc::Receiver<AgentEvent>) {
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
        Box::new(AutoDenyHandler),
        Box::new(AutoCancelQuestionHandler),
    ));
    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());

    let mut routing = HashMap::new();
    routing.insert(TaskType::Summarization, summarization_model.to_string());
    let router = ModelRouter::from_parts("claude-sonnet-4-20250514".into(), routing);

    let params = AgentLoopParams {
        config: AgentConfig {
            router,
            ..Default::default()
        },
        deps: AgentDeps {
            kernel,
            frontend,
            session_manager: fixture.session_manager(),
        },
        session: fixture.test_session("test-routing"),
        store: ContextStore::new(make_test_budget()),
        interrupt: InterruptHandle::new(),
        shared: None,
        memory_channel: None,
        scheduled_rx: None,
    };
    (AgentLoopRunner::new(params), event_rx)
}

#[test]
fn test_router_resolves_default_model() {
    let (runner, _rx) = make_runner_with_routing("claude-haiku-3-5-20241022");

    assert_eq!(runner.params.config.model(), "claude-sonnet-4-20250514");
    assert_eq!(
        runner.params.config.router.resolve(TaskType::Default),
        "claude-sonnet-4-20250514"
    );
}

#[test]
fn test_router_resolves_summarization_override() {
    let (runner, _rx) = make_runner_with_routing("claude-haiku-3-5-20241022");

    assert_eq!(
        runner.params.config.router.resolve(TaskType::Summarization),
        "claude-haiku-3-5-20241022"
    );
}

#[test]
fn test_model_switch_preserves_summarization_override() {
    let (mut runner, _rx) = make_runner_with_routing("claude-haiku-3-5-20241022");

    // Switch the default model
    runner
        .params
        .config
        .router
        .set_default("claude-opus-4-6".into());

    // Default changed
    assert_eq!(runner.params.config.model(), "claude-opus-4-6");
    // Summarization override untouched
    assert_eq!(
        runner.params.config.router.resolve(TaskType::Summarization),
        "claude-haiku-3-5-20241022"
    );
}

#[test]
fn test_model_routing_default_override_via_config_model() {
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
        Box::new(AutoDenyHandler),
        Box::new(AutoCancelQuestionHandler),
    ));
    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());

    // User sets model_routing.default to override the base model
    let mut routing = HashMap::new();
    routing.insert(TaskType::Default, "claude-opus-4-6".into());
    let router = ModelRouter::from_parts("claude-sonnet-4-20250514".into(), routing);

    let params = AgentLoopParams {
        config: AgentConfig {
            router,
            ..Default::default()
        },
        deps: AgentDeps {
            kernel,
            frontend,
            session_manager: fixture.session_manager(),
        },
        session: fixture.test_session("test-default-override"),
        store: ContextStore::new(make_test_budget()),
        interrupt: InterruptHandle::new(),
        shared: None,
        memory_channel: None,
        scheduled_rx: None,
    };
    let (runner, _rx) = (AgentLoopRunner::new(params), event_rx);

    // config.model() should respect model_routing.default override
    assert_eq!(runner.params.config.model(), "claude-opus-4-6");
}
