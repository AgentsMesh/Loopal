//! Unit-style tests for `AgentLoopParamsBuilder` defaults and chained
//! setters, plus `SessionResumeError` constructor and `AgentLoopParams`
//! getter accessors.

use std::sync::Arc;

use loopal_runtime::{
    AgentConfig, AgentDeps, AgentLoopParamsBuilder, InterruptHandle, SessionResumeError,
    SessionResumeHook,
};
use loopal_test_support::TestFixture;

use async_trait::async_trait;

struct NoopHook;
#[async_trait]
impl SessionResumeHook for NoopHook {
    async fn on_session_changed(&self, _new: &str) -> Result<(), SessionResumeError> {
        Ok(())
    }
}

fn deps_for(fixture: &TestFixture) -> AgentDeps {
    use loopal_runtime::frontend::{AutoCancelQuestionHandler, AutoDenyHandler, UnifiedFrontend};
    let (event_tx, _event_rx) = tokio::sync::mpsc::channel(8);
    let (_mbox_tx, mbox_rx) = tokio::sync::mpsc::channel(8);
    let (_ctrl_tx, ctrl_rx) = tokio::sync::mpsc::channel(8);
    let frontend = Arc::new(UnifiedFrontend::new(
        None,
        event_tx,
        mbox_rx,
        ctrl_rx,
        None,
        Box::new(AutoDenyHandler),
        Box::new(AutoCancelQuestionHandler),
    ));
    AgentDeps {
        kernel: Arc::new(loopal_kernel::Kernel::new(Default::default()).unwrap()),
        frontend,
        session_manager: fixture.session_manager(),
    }
}

#[test]
fn session_resume_error_carries_hook_name_and_reason() {
    let err = SessionResumeError::new("cron", "disk full");
    let s = err.to_string();
    assert!(s.contains("cron"));
    assert!(s.contains("disk full"));
}

#[tokio::test]
async fn builder_default_optionals_yield_none_or_empty() {
    let fixture = TestFixture::new();
    let session = fixture.test_session("primary");
    let store = loopal_context::ContextStore::new(loopal_context::ContextBudget {
        context_window: 1000,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: 16,
        safety_margin: 16,
        message_budget: 952,
        max_output_tokens: 64,
    });
    let params = AgentLoopParamsBuilder::new(
        AgentConfig::default(),
        deps_for(&fixture),
        session,
        store,
        InterruptHandle::new(),
    )
    .build();
    assert!(params.shared.is_none());
    assert!(params.memory_channel.is_none());
    assert!(params.scheduled_rx.is_none());
    assert!(params.auto_classifier.is_none());
    assert!(params.rewake_rx.is_none());
    assert!(params.message_snapshot.is_none());
    assert!(params.resume_hooks.is_empty());
}

#[tokio::test]
async fn builder_chained_setters_override_defaults() {
    let fixture = TestFixture::new();
    let session = fixture.test_session("primary");
    let store = loopal_context::ContextStore::new(loopal_context::ContextBudget {
        context_window: 1000,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: 16,
        safety_margin: 16,
        message_budget: 952,
        max_output_tokens: 64,
    });
    let hook: Arc<dyn SessionResumeHook> = Arc::new(NoopHook);
    let params = AgentLoopParamsBuilder::new(
        AgentConfig::default(),
        deps_for(&fixture),
        session,
        store,
        InterruptHandle::new(),
    )
    .resume_hooks(vec![hook.clone()])
    .build();
    assert_eq!(params.resume_hooks.len(), 1);
}

#[tokio::test]
async fn agent_loop_params_getter_session_returns_constructed_session() {
    let fixture = TestFixture::new();
    let session = fixture.test_session("named-session");
    let store = loopal_context::ContextStore::new(loopal_context::ContextBudget {
        context_window: 1000,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: 16,
        safety_margin: 16,
        message_budget: 952,
        max_output_tokens: 64,
    });
    let params = AgentLoopParamsBuilder::new(
        AgentConfig::default(),
        deps_for(&fixture),
        session,
        store,
        InterruptHandle::new(),
    )
    .build();
    assert_eq!(params.session().id, "named-session");
}

#[tokio::test]
async fn agent_loop_params_getter_config_returns_constructed_config() {
    let fixture = TestFixture::new();
    let session = fixture.test_session("s");
    let store = loopal_context::ContextStore::new(loopal_context::ContextBudget {
        context_window: 1000,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: 16,
        safety_margin: 16,
        message_budget: 952,
        max_output_tokens: 64,
    });
    let params = AgentLoopParamsBuilder::new(
        AgentConfig {
            permission_mode: loopal_tool_api::PermissionMode::Bypass,
            ..Default::default()
        },
        deps_for(&fixture),
        session,
        store,
        InterruptHandle::new(),
    )
    .build();
    assert_eq!(
        params.config().permission_mode,
        loopal_tool_api::PermissionMode::Bypass
    );
}
