use std::sync::Arc;
use std::time::Duration;

use loopal_protocol::AgentEventPayload;
use loopal_protocol::ControlCommand;
use loopal_protocol::Envelope;

use super::{make_runner, make_runner_with_channels};

#[test]
fn test_model_info_defaults_for_unknown_model() {
    use loopal_config::Settings;
    use loopal_kernel::Kernel;
    use loopal_runtime::agent_loop::AgentLoopRunner;
    use loopal_runtime::frontend::{DenyAllHandler, UnsupportedQuestionHandler};
    use loopal_runtime::{
        AgentConfig, AgentDeps, AgentLoopParamsBuilder, InterruptHandle, UnifiedFrontend,
    };
    use loopal_test_support::TestFixture;
    use loopal_tool_api::PermissionMode;
    use tokio::sync::mpsc;

    let fixture = TestFixture::new();
    let (event_tx, _event_rx) = mpsc::channel(16);
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
        AgentConfig {
            router: loopal_provider_api::SharedModelRouter::with_default(
                "unknown-model-xyz".to_string(),
            ),
            permission_mode: PermissionMode::AskAnyWrite,
            ..Default::default()
        },
        AgentDeps {
            kernel,
            frontend,
            session_manager: fixture.session_manager(),
            decision_context: loopal_runtime::frontend::DecisionContext::with_cwd("/tmp/test"),
            protected_effect_audit: super::noop_protected_effect_audit(),
        },
        fixture.test_session("test"),
        super::make_test_budget(),
        InterruptHandle::new(),
    )
    .build();

    let runner = AgentLoopRunner::new(params);
    // Unknown model should fall back to defaults
    assert_eq!(runner.model_config.max_context_tokens, 200_000);
}

#[tokio::test]
async fn test_emit_multiple_events() {
    let (runner, mut rx) = make_runner();

    runner.emit(AgentEventPayload::Started).await.unwrap();
    runner
        .emit(AgentEventPayload::Stream {
            text: "hello".to_string(),
        })
        .await
        .unwrap();
    runner.emit(AgentEventPayload::Finished).await.unwrap();

    assert!(matches!(
        rx.recv().await.unwrap().payload,
        AgentEventPayload::Started
    ));
    assert!(
        matches!(rx.recv().await.unwrap().payload, AgentEventPayload::Stream { ref text } if text == "hello")
    );
    assert!(matches!(
        rx.recv().await.unwrap().payload,
        AgentEventPayload::Finished
    ));
}

// --- handle_control behavior tests ---

#[tokio::test]
async fn test_handle_control_model_switch_updates_model() {
    let (mut runner, _event_rx, _mbox_tx, ctrl_tx, _perm_tx) = make_runner_with_channels();

    assert_eq!(runner.params.config.model(), "claude-sonnet-4-20250514");

    ctrl_tx
        .send(ControlCommand::ModelSwitch("claude-opus-4-20250514".into()))
        .await
        .unwrap();
    drop(ctrl_tx);

    let _ = tokio::time::timeout(Duration::from_millis(100), runner.wait_for_input()).await;

    assert_eq!(runner.params.config.model(), "claude-opus-4-20250514");
}
