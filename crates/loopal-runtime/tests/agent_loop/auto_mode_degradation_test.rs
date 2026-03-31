//! Auto Mode integration tests: degradation fallback and error recovery.

use std::sync::Arc;

use loopal_runtime::agent_loop::AgentLoopRunner;

use super::auto_mode_helpers::*;
use super::make_cancel;

// ── Degradation: circuit breaker tripped ───────────────────────────

/// Degraded → falls back to frontend permission handler (AutoDenyHandler → Deny).
#[tokio::test]
async fn degraded_falls_back_to_frontend() {
    let (mut runner, _event_rx) = make_auto_runner(vec![]);

    force_degrade(&runner).await;

    let tool_uses = vec![(
        "tc-1".into(),
        "DangerTool".into(),
        serde_json::json!({"command": "echo hi"}),
    )];

    // Must complete within 2s — hang = test failure.
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        runner.execute_tools(tool_uses, &make_cancel()),
    )
    .await;
    assert!(result.is_ok(), "degraded must not hang");
}

/// Degraded + AutoDenyHandler → denied (exercises the human fallback path).
#[tokio::test]
async fn degraded_with_auto_deny_handler() {
    let (mut runner, _event_rx) = make_auto_runner(vec![]);

    force_degrade(&runner).await;

    let tool_uses = vec![(
        "tc-1".into(),
        "DangerTool".into(),
        serde_json::json!({"command": "echo hi"}),
    )];

    // AutoDenyHandler denies → tool denied.
    runner
        .execute_tools(tool_uses, &make_cancel())
        .await
        .unwrap();

    let msg = &runner.params.store.messages()[0];
    match &msg.content[0] {
        loopal_message::ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            assert!(is_error);
            assert!(content.contains("Permission denied"), "got: {content}");
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }
}

// ── Provider failure ───────────────────────────────────────────────

/// No provider registered → deny all gracefully (no panic).
#[tokio::test]
async fn no_provider_denies_gracefully() {
    use loopal_auto_mode::AutoClassifier;
    use loopal_config::Settings;
    use loopal_context::ContextStore;
    use loopal_kernel::Kernel;
    use loopal_protocol::{ControlCommand, Envelope};
    use loopal_runtime::agent_loop::AgentLoopRunner;
    use loopal_runtime::frontend::{AutoCancelQuestionHandler, AutoDenyHandler};
    use loopal_runtime::{
        AgentConfig, AgentDeps, AgentLoopParams, InterruptHandle, UnifiedFrontend,
    };
    use loopal_test_support::TestFixture;
    use loopal_tool_api::PermissionMode;
    use tokio::sync::mpsc;

    let fixture = TestFixture::new();
    let (event_tx, _event_rx) = mpsc::channel(64);
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
    // Kernel with DangerTool but NO provider.
    let mut kernel = Kernel::new(Settings::default()).unwrap();
    kernel.register_tool(Box::new(DummyTool::dangerous("DangerTool")));
    let classifier = Arc::new(AutoClassifier::new(String::new(), "/tmp".into()));
    let params = AgentLoopParams {
        config: AgentConfig {
            permission_mode: PermissionMode::Auto,
            ..Default::default()
        },
        deps: AgentDeps {
            kernel: Arc::new(kernel),
            frontend,
            session_manager: fixture.session_manager(),
        },
        session: fixture.test_session("no-provider"),
        store: ContextStore::new(super::make_test_budget()),
        interrupt: InterruptHandle::new(),
        shared: None,
        memory_channel: None,
        scheduled_rx: None,
        auto_classifier: Some(classifier),
    };
    let mut runner = AgentLoopRunner::new(params);

    let tool_uses = vec![(
        "tc-1".into(),
        "DangerTool".into(),
        serde_json::json!({"command": "echo test"}),
    )];

    let result = runner.execute_tools(tool_uses, &make_cancel()).await;
    assert!(result.is_ok(), "provider failure should not crash the turn");
}

// ── Helper ─────────────────────────────────────────────────────────

/// Force the classifier's circuit breaker into degraded state.
async fn force_degrade(runner: &AgentLoopRunner) {
    let classifier = runner.params.auto_classifier.as_ref().unwrap();
    let err_provider = Arc::new(ErrProvider);
    for i in 0..20 {
        classifier
            .classify(
                &format!("T{i}"),
                &serde_json::json!({}),
                "",
                &*err_provider,
                "m",
            )
            .await;
    }
    assert!(classifier.is_degraded());
}
