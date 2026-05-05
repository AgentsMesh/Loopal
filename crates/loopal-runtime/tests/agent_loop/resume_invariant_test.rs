use loopal_config::Settings;
use loopal_context::ContextStore;
use loopal_error::LoopalError;
use loopal_kernel::Kernel;
use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_protocol::{AgentEvent, ControlCommand, Envelope};
use loopal_provider_api::{ChatParams, ChatStream, Provider, StopReason, StreamChunk};
use loopal_runtime::agent_loop::AgentLoopRunner;
use loopal_runtime::frontend::{AutoCancelQuestionHandler, AutoDenyHandler};
use loopal_runtime::{
    AgentConfig, AgentDeps, AgentLoopParams, AgentLoopParamsBuilder, InterruptHandle,
    UnifiedFrontend,
};
use loopal_test_support::TestFixture;
use loopal_test_support::mock_provider::MockStreamChunks;
use loopal_tool_api::PermissionMode;
use std::collections::VecDeque;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::mpsc;

use super::make_test_budget;

struct CountingProvider {
    call_count: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl Provider for CountingProvider {
    fn name(&self) -> &str {
        "anthropic"
    }
    async fn stream_chat(&self, _p: &ChatParams) -> Result<ChatStream, LoopalError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let chunks = vec![
            Ok(StreamChunk::Text { text: "ok".into() }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ];
        Ok(Box::pin(MockStreamChunks::new(VecDeque::from(chunks))))
    }
}

fn assistant(text: &str) -> Message {
    Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![ContentBlock::Text { text: text.into() }],
    }
}

fn user(text: &str) -> Message {
    Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::Text { text: text.into() }],
    }
}

fn make_runner_with_history(
    history: Vec<Message>,
) -> (
    AgentLoopRunner,
    Arc<AtomicUsize>,
    mpsc::Receiver<AgentEvent>,
) {
    let fixture = TestFixture::new();
    let (event_tx, event_rx) = mpsc::channel::<AgentEvent>(64);
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
    let call_count = Arc::new(AtomicUsize::new(0));
    let provider = CountingProvider {
        call_count: Arc::clone(&call_count),
    };
    kernel.register_provider(Arc::new(provider) as Arc<dyn Provider>);
    let params: AgentLoopParams = AgentLoopParamsBuilder::new(
        AgentConfig {
            permission_mode: PermissionMode::Bypass,
            ..Default::default()
        },
        AgentDeps {
            kernel: Arc::new(kernel),
            frontend,
            session_manager: fixture.session_manager(),
        },
        fixture.test_session("rt-test"),
        ContextStore::from_messages(history, make_test_budget()),
        InterruptHandle::new(),
    )
    .build();
    (AgentLoopRunner::new(params), call_count, event_rx)
}

#[tokio::test]
async fn resume_with_assistant_tail_does_not_call_llm() {
    // Simulate session resume after crash where the last persisted message
    // is an Assistant text response (turn finished but no User input followed).
    // Pre-fix bug: run_loop's `needs_input = store.is_empty()` was false →
    // skipped idle phase → ReadyToCall debug_assert panicked / release silently
    // sent assistant-tailed messages to the LLM.
    let history = vec![user("hello"), assistant("hi there")];
    let (mut runner, calls, mut rx) = make_runner_with_history(history);
    tokio::spawn(async move { while rx.recv().await.is_some() {} });

    let _ = runner.run().await.unwrap();

    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "agent must wait for user input when store last role is Assistant; \
         observed LLM calls={} indicates the idle phase was skipped",
        calls.load(Ordering::SeqCst)
    );
}

#[tokio::test]
async fn resume_with_user_tail_calls_llm_immediately() {
    // Sanity: when last message is a User (e.g. tool_result mid-turn), the
    // agent should resume the turn without waiting for further input.
    let history = vec![user("question")];
    let (mut runner, calls, mut rx) = make_runner_with_history(history);
    tokio::spawn(async move { while rx.recv().await.is_some() {} });

    let _ = runner.run().await.unwrap();

    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "User tail should trigger immediate LLM call to resume the turn"
    );
}

#[tokio::test]
async fn resume_with_empty_store_waits_for_input() {
    let (mut runner, calls, mut rx) = make_runner_with_history(vec![]);
    tokio::spawn(async move { while rx.recv().await.is_some() {} });

    let _ = runner.run().await.unwrap();

    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "empty store must wait for user input"
    );
}
