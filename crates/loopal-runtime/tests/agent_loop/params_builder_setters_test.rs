use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use loopal_config::HarnessConfig;
use loopal_decision_api::DecisionMode;
use loopal_protocol::{AgentEventPayload, ControlCommand, Envelope};
use loopal_provider_api::Message;
use loopal_runtime::frontend::{
    DecisionCell, DenyAllHandler, EventEmitter, UnifiedFrontend, UnsupportedQuestionHandler,
};
use loopal_runtime::{
    AgentConfig, AgentDeps, AgentLoopParamsBuilder, GoalRuntimeSession, InterruptHandle,
};
use loopal_storage::GoalStore;
use loopal_test_support::TestFixture;
use loopal_tool_api::{FetchRefinerPolicy, MemoryChannel, OneShotChatError, OneShotChatService};

struct MemoryStub;

impl MemoryChannel for MemoryStub {
    fn try_send(&self, _: String) -> Result<(), String> {
        Ok(())
    }
}

struct ChatStub;

#[async_trait]
impl OneShotChatService for ChatStub {
    async fn one_shot_chat(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: u32,
    ) -> Result<String, OneShotChatError> {
        Ok("ok".into())
    }
}

struct RefinerStub;

impl FetchRefinerPolicy for RefinerStub {
    fn refiner_model(&self, _: usize) -> Option<String> {
        Some("test-model".into())
    }
}

struct NoopEmitter;

#[async_trait]
impl EventEmitter for NoopEmitter {
    async fn emit(&self, _: AgentEventPayload) -> loopal_error::Result<()> {
        Ok(())
    }
}

fn deps(fixture: &TestFixture) -> AgentDeps {
    let (event_tx, _event_rx) = tokio::sync::mpsc::channel(8);
    let (_mailbox_tx, mailbox_rx) = tokio::sync::mpsc::channel::<Envelope>(8);
    let (_control_tx, control_rx) = tokio::sync::mpsc::channel::<ControlCommand>(8);
    let frontend = Arc::new(UnifiedFrontend::new(
        None,
        event_tx,
        mailbox_rx,
        control_rx,
        None,
        Box::new(DenyAllHandler),
        Box::new(UnsupportedQuestionHandler),
    ));
    AgentDeps {
        kernel: Arc::new(loopal_kernel::Kernel::new(Default::default()).unwrap()),
        frontend,
        session_manager: fixture.session_manager(),
        decision_context: loopal_runtime::frontend::DecisionContext::with_cwd("/tmp/test"),
        protected_effect_audit: super::noop_protected_effect_audit(),
    }
}

#[test]
fn builder_sets_runtime_services_and_channels() {
    let fixture = TestFixture::new();
    let goal_dir = tempfile::tempdir().unwrap();
    let memory: Arc<dyn MemoryChannel> = Arc::new(MemoryStub);
    let chat: Arc<dyn OneShotChatService> = Arc::new(ChatStub);
    let refiner: Arc<dyn FetchRefinerPolicy> = Arc::new(RefinerStub);
    let goal = Arc::new(GoalRuntimeSession::new(
        "goal-session".into(),
        Arc::new(GoalStore::with_base_dir(goal_dir.path().to_path_buf())),
        Box::new(NoopEmitter),
    ));
    let (_rewake_tx, rewake_rx) = tokio::sync::mpsc::channel(1);
    let snapshot = Arc::new(RwLock::new(vec![Message::user("snapshot")]));
    let decision = DecisionCell::new(DecisionMode::Classifier);
    let harness = HarnessConfig {
        loop_warn_threshold: 17,
        ..Default::default()
    };

    let params = AgentLoopParamsBuilder::new(
        AgentConfig::default(),
        deps(&fixture),
        fixture.test_session("builder-services"),
        super::make_test_budget(),
        InterruptHandle::new(),
    )
    .memory_channel(memory.clone())
    .memory_channel_opt(Some(memory.clone()))
    .one_shot_chat(chat.clone())
    .fetch_refiner_policy(refiner.clone())
    .goal_session(goal.clone())
    .harness(harness)
    .rewake_rx(rewake_rx)
    .message_snapshot(snapshot.clone())
    .decision_cell(decision.clone())
    .build();

    assert!(Arc::ptr_eq(
        params.memory_channel.as_ref().unwrap(),
        &memory
    ));
    assert!(Arc::ptr_eq(params.one_shot_chat.as_ref().unwrap(), &chat));
    assert!(Arc::ptr_eq(
        params.fetch_refiner_policy.as_ref().unwrap(),
        &refiner
    ));
    assert!(Arc::ptr_eq(params.goal_session.as_ref().unwrap(), &goal));
    assert_eq!(params.harness.loop_warn_threshold, 17);
    assert!(params.rewake_rx.is_some());
    assert!(Arc::ptr_eq(
        params.message_snapshot.as_ref().unwrap(),
        &snapshot
    ));
    decision.set(DecisionMode::Agent);
    assert_eq!(params.decision_cell.get(), DecisionMode::Agent);
}
