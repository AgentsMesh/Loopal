//! Shared helpers for Auto Mode integration tests.

use std::sync::Arc;

use loopal_auto_mode::AutoClassifier;
use loopal_config::Settings;
use loopal_context::ContextStore;
use loopal_kernel::Kernel;
use loopal_protocol::{AgentEvent, AgentEventPayload, ControlCommand, Envelope};
use loopal_provider_api::{Provider, StopReason, StreamChunk};
use loopal_runtime::agent_loop::AgentLoopRunner;
use loopal_runtime::frontend::{AutoCancelQuestionHandler, AutoDenyHandler};
use loopal_runtime::{AgentConfig, AgentDeps, AgentLoopParams, InterruptHandle, UnifiedFrontend};
use loopal_test_support::TestFixture;
use loopal_test_support::mock_provider::MultiCallProvider;
use loopal_tool_api::{PermissionLevel, PermissionMode, Tool, ToolContext, ToolResult};
use tokio::sync::mpsc;

use super::make_test_budget;

/// Classifier JSON response that allows the tool.
pub fn allow_chunks() -> Vec<Result<StreamChunk, loopal_error::LoopalError>> {
    vec![
        Ok(StreamChunk::Text {
            text: r#"{"should_block": false, "reason": "safe operation"}"#.into(),
        }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        }),
    ]
}

/// Classifier JSON response that denies the tool.
pub fn deny_chunks() -> Vec<Result<StreamChunk, loopal_error::LoopalError>> {
    vec![
        Ok(StreamChunk::Text {
            text: r#"{"should_block": true, "reason": "dangerous command"}"#.into(),
        }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        }),
    ]
}

/// Dummy tool with configurable PermissionLevel for testing permission paths.
pub struct DummyTool {
    name: &'static str,
    perm: PermissionLevel,
}

impl DummyTool {
    pub fn dangerous(name: &'static str) -> Self {
        Self {
            name,
            perm: PermissionLevel::Dangerous,
        }
    }
}

#[async_trait::async_trait]
impl Tool for DummyTool {
    fn name(&self) -> &str {
        self.name
    }
    fn description(&self) -> &str {
        "dummy tool for testing"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }
    fn permission(&self) -> PermissionLevel {
        self.perm
    }
    async fn execute(
        &self,
        _input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, loopal_error::LoopalError> {
        Ok(ToolResult::success("ok"))
    }
}

/// Build a runner in Auto mode with a MultiCallProvider for classifier responses.
/// Registers a "DangerTool" with Dangerous permission by default.
pub fn make_auto_runner(
    classifier_calls: Vec<Vec<Result<StreamChunk, loopal_error::LoopalError>>>,
) -> (AgentLoopRunner, mpsc::Receiver<AgentEvent>) {
    make_auto_runner_with_setup(classifier_calls, |kernel| {
        kernel.register_tool(Box::new(DummyTool::dangerous("DangerTool")));
    })
}

/// Build a runner in Auto mode with custom kernel setup.
pub fn make_auto_runner_with_setup(
    classifier_calls: Vec<Vec<Result<StreamChunk, loopal_error::LoopalError>>>,
    setup: impl FnOnce(&mut Kernel),
) -> (AgentLoopRunner, mpsc::Receiver<AgentEvent>) {
    let fixture = TestFixture::new();
    let (event_tx, event_rx) = mpsc::channel(64);
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
    let mut kernel = Kernel::new(Settings::default()).unwrap();
    let provider = Arc::new(MultiCallProvider::new(classifier_calls)) as Arc<dyn Provider>;
    kernel.register_provider(provider);
    setup(&mut kernel);
    let classifier = Arc::new(AutoClassifier::new(String::new(), "/tmp/test".into()));
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
        session: fixture.test_session("auto-mode-test"),
        store: ContextStore::new(make_test_budget()),
        interrupt: InterruptHandle::new(),
        shared: None,
        memory_channel: None,
        scheduled_rx: None,
        auto_classifier: Some(classifier),
    };
    (AgentLoopRunner::new(params), event_rx)
}

/// Drain events and return all AutoModeDecision payloads as (tool_name, decision).
pub fn drain_auto_decisions(rx: &mut mpsc::Receiver<AgentEvent>) -> Vec<(String, String)> {
    let mut decisions = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let AgentEventPayload::AutoModeDecision {
            tool_name,
            decision,
            ..
        } = event.payload
        {
            decisions.push((tool_name, decision));
        }
    }
    decisions
}

/// Provider that always returns errors (for forcing classifier degradation).
pub struct ErrProvider;

#[async_trait::async_trait]
impl Provider for ErrProvider {
    fn name(&self) -> &str {
        "mock"
    }
    async fn stream_chat(
        &self,
        _p: &loopal_provider_api::ChatParams,
    ) -> Result<loopal_provider_api::ChatStream, loopal_error::LoopalError> {
        Err(loopal_error::LoopalError::Other("mock error".into()))
    }
}
