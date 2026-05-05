use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use loopal_config::Settings;
use loopal_error::{LoopalError, ProviderError};
use loopal_kernel::Kernel;
use loopal_protocol::{AgentEvent, ControlCommand, Envelope};
use loopal_provider_api::{
    ChatParams, ChatStream, ContinuationIntent, ErrorClass, Provider, StopReason, StreamChunk,
    default_classify_error,
};
use loopal_runtime::agent_loop::AgentLoopRunner;
use loopal_runtime::frontend::{AutoCancelQuestionHandler, AutoDenyHandler};
use loopal_runtime::{
    AgentConfig, AgentDeps, AgentLoopParams, AgentLoopParamsBuilder, InterruptHandle,
    UnifiedFrontend,
};
use loopal_test_support::TestFixture;
use loopal_test_support::mock_provider::MockStreamChunks;
use loopal_tool_api::PermissionMode;
use tokio::sync::mpsc;

use super::make_test_budget;

pub enum Outcome {
    Err(LoopalError),
    Stream(Vec<Result<StreamChunk, LoopalError>>),
}

pub type IntentLog = Arc<Mutex<Vec<Option<ContinuationIntent>>>>;

pub struct SequencedProvider {
    outcomes: Mutex<VecDeque<Outcome>>,
    call_count: Arc<AtomicUsize>,
    intents: IntentLog,
}

impl SequencedProvider {
    pub fn new(outcomes: Vec<Outcome>) -> (Self, Arc<AtomicUsize>, IntentLog) {
        let count = Arc::new(AtomicUsize::new(0));
        let intents: IntentLog = Arc::new(Mutex::new(Vec::new()));
        let p = Self {
            outcomes: Mutex::new(VecDeque::from(outcomes)),
            call_count: Arc::clone(&count),
            intents: Arc::clone(&intents),
        };
        (p, count, intents)
    }
}

#[async_trait::async_trait]
impl Provider for SequencedProvider {
    fn name(&self) -> &str {
        "anthropic"
    }
    async fn stream_chat(&self, p: &ChatParams) -> Result<ChatStream, LoopalError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        self.intents
            .lock()
            .unwrap()
            .push(p.continuation_intent.clone());
        let outcome = self
            .outcomes
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Outcome::Stream(vec![]));
        match outcome {
            Outcome::Err(e) => Err(e),
            Outcome::Stream(chunks) => Ok(Box::pin(MockStreamChunks::new(VecDeque::from(chunks)))),
        }
    }
    fn classify_error(&self, err: &LoopalError) -> ErrorClass {
        // Mirror anthropic provider keyword classification so try_recover
        // dispatches the same recovery path as production.
        if let LoopalError::Provider(ProviderError::Api {
            status: 400,
            message,
        }) = err
        {
            if message.contains("does not support assistant message prefill") {
                return ErrorClass::PrefillRejected;
            }
            if message.contains("code_execution") && message.contains("without a corresponding") {
                return ErrorClass::ServerBlockError;
            }
        }
        default_classify_error(err)
    }
}

pub fn make_runner(
    outcomes: Vec<Outcome>,
) -> (
    AgentLoopRunner,
    Arc<AtomicUsize>,
    mpsc::Receiver<AgentEvent>,
) {
    let (runner, count, _intents, rx) = make_runner_with_intents(outcomes);
    (runner, count, rx)
}

pub fn make_runner_with_intents(
    outcomes: Vec<Outcome>,
) -> (
    AgentLoopRunner,
    Arc<AtomicUsize>,
    IntentLog,
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
    let (provider, call_count, intents) = SequencedProvider::new(outcomes);
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
        loopal_context::ContextStore::from_messages(
            vec![loopal_message::Message::user("go")],
            make_test_budget(),
        ),
        InterruptHandle::new(),
    )
    .build();
    (AgentLoopRunner::new(params), call_count, intents, event_rx)
}

pub fn ok_done() -> Vec<Result<StreamChunk, LoopalError>> {
    vec![
        Ok(StreamChunk::Text { text: "ok".into() }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        }),
    ]
}

pub fn server_block_err() -> LoopalError {
    LoopalError::Provider(ProviderError::Api {
        status: 400,
        message: "code_execution server block without a corresponding tool_result".into(),
    })
}

pub fn context_overflow_err() -> LoopalError {
    LoopalError::Provider(ProviderError::ContextOverflow {
        message: "prompt is too long".into(),
    })
}

pub fn prefill_rejection_err() -> LoopalError {
    LoopalError::Provider(ProviderError::Api {
        status: 400,
        message: "This model does not support assistant message prefill.".into(),
    })
}
