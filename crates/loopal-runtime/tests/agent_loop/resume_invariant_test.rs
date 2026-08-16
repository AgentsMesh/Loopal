use loopal_config::Settings;
use loopal_error::LoopalError;
use loopal_kernel::Kernel;
use loopal_protocol::{AgentEvent, ControlCommand, Envelope, MessageSource};
use loopal_provider_api::{ChatParams, ChatStream, Provider, StopReason, StreamChunk};
use loopal_provider_api::{ContentBlock, Message, MessageRole};
use loopal_runtime::agent_loop::AgentLoopRunner;
use loopal_runtime::frontend::{DenyAllHandler, UnsupportedQuestionHandler};
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
        origin: None,
        ephemeral_in_history: false,
    }
}

fn user(text: &str) -> Message {
    Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::Text { text: text.into() }],
        origin: None,
        ephemeral_in_history: false,
    }
}

fn make_runner_with_history(
    history: Vec<Message>,
) -> (
    AgentLoopRunner,
    Arc<AtomicUsize>,
    mpsc::Receiver<AgentEvent>,
    mpsc::Sender<Envelope>,
    mpsc::Sender<ControlCommand>,
) {
    let fixture = TestFixture::new();
    let (event_tx, event_rx) = mpsc::channel::<AgentEvent>(64);
    let (mbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(16);
    let (ctrl_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let frontend = Arc::new(UnifiedFrontend::new(
        None,
        event_tx,
        mailbox_rx,
        control_rx,
        None,
        Box::new(DenyAllHandler),
        Box::new(UnsupportedQuestionHandler),
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
            decision_context: loopal_runtime::frontend::DecisionContext::with_cwd("/tmp/test"),
            protected_effect_audit: super::noop_protected_effect_audit(),
        },
        fixture.test_session("rt-test"),
        make_test_budget(),
        InterruptHandle::new(),
    )
    .build();
    let mut runner = AgentLoopRunner::new(params);
    let turns = loopal_test_support::seed_history::reverse_project_messages_to_turns(history);
    runner.seed_test_turns(turns);
    (runner, call_count, event_rx, mbox_tx, ctrl_tx)
}

#[tokio::test]
async fn resume_with_assistant_tail_does_not_call_llm() {
    // Simulate session resume after crash where the last persisted message
    // is an Assistant text response (turn finished but no User input followed).
    // Pre-fix bug: run_loop's `needs_input = store.is_empty()` was false →
    // skipped idle phase → ReadyToCall debug_assert panicked / release silently
    // sent assistant-tailed messages to the LLM.
    let history = vec![user("hello"), assistant("hi there")];
    let (mut runner, calls, mut rx, mbox_tx, ctrl_tx) = make_runner_with_history(history);
    drop(mbox_tx);
    drop(ctrl_tx);
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
async fn resume_with_completed_user_tail_waits_for_input() {
    // A workflow-handled input is a completed user-only turn. Its User tail
    // remains in history for future context, but must not be executed again.
    let history = vec![user("question")];
    let (mut runner, calls, mut rx, mbox_tx, ctrl_tx) = make_runner_with_history(history);
    drop(mbox_tx);
    drop(ctrl_tx);
    tokio::spawn(async move { while rx.recv().await.is_some() {} });

    let _ = runner.run().await.unwrap();

    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "a completed user-only turn must be idle after resume"
    );
}

#[tokio::test]
async fn resume_with_empty_store_waits_for_input() {
    let (mut runner, calls, mut rx, mbox_tx, ctrl_tx) = make_runner_with_history(vec![]);
    drop(mbox_tx);
    drop(ctrl_tx);
    tokio::spawn(async move { while rx.recv().await.is_some() {} });

    let _ = runner.run().await.unwrap();

    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "empty store must wait for user input"
    );
}

#[tokio::test]
async fn crash_recovered_user_tail_resumes_and_records_llm_step() {
    let (mut runner, calls, mut rx, mbox_tx, ctrl_tx) = make_runner_with_history(vec![]);
    let mut recovered = loopal_turn::Turn::new(loopal_turn::TurnTrigger::UserInput {
        envelope_id: "crashed-envelope".into(),
        content: "question".into(),
        images: Vec::new(),
    });
    recovered.outcome = loopal_turn::TurnOutcome::Cancelled {
        cause: loopal_turn::CancelledCause::CrashRecovery,
    };
    runner.seed_test_turns(vec![recovered]);
    drop(mbox_tx);
    drop(ctrl_tx);
    tokio::spawn(async move { while rx.recv().await.is_some() {} });

    let _ = runner.run().await.unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let has_llm_step = runner
        .recorded_turns()
        .iter()
        .flat_map(|t| &t.body.steps)
        .any(|s| matches!(s, loopal_turn::TurnStep::LlmCall { .. }));
    assert!(
        has_llm_step,
        "crash-recovered input must open a resume record for its LlmCall",
    );
}

#[tokio::test]
async fn completed_user_tail_accepts_a_new_followup_turn() {
    // The retained user-only history stays visible, while only the newly
    // delivered envelope is dispatched after resume.
    let history = vec![user("question")];
    let (mut runner, calls, mut rx, mbox_tx, ctrl_tx) = make_runner_with_history(history);

    tokio::spawn(async move {
        let mut idle_count = 0;
        while let Some(ev) = rx.recv().await {
            if matches!(
                ev.payload,
                loopal_protocol::AgentEventPayload::AwaitingInput
            ) {
                idle_count += 1;
                if idle_count == 1 {
                    mbox_tx
                        .send(Envelope::new(MessageSource::Human, "main", "continue"))
                        .await
                        .ok();
                } else {
                    drop(mbox_tx);
                    drop(ctrl_tx);
                    break;
                }
            }
        }
        while rx.recv().await.is_some() {}
    });

    let _ = runner.run().await.unwrap();

    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "only the newly delivered followup should call the provider",
    );
}
