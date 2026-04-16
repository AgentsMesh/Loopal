//! Tests for natural turn completion flow through execute_turn.
use std::collections::VecDeque;
use std::sync::Arc;

use async_trait::async_trait;
use futures::stream::Stream as FutStream;
use loopal_config::Settings;
use loopal_context::{ContextBudget, ContextStore};
use loopal_error::LoopalError;
use loopal_kernel::Kernel;
use loopal_protocol::ControlCommand;
use loopal_protocol::Envelope;
use loopal_provider_api::{ChatParams, ChatStream, Provider, StreamChunk};
use loopal_runtime::agent_loop::AgentLoopRunner;
use loopal_runtime::frontend::{AutoCancelQuestionHandler, AutoDenyHandler};
use loopal_runtime::{AgentConfig, AgentDeps, AgentLoopParams, InterruptHandle, UnifiedFrontend};
use loopal_test_support::TestFixture;
use tokio::sync::mpsc;

// --- Multi-call mock provider ---
pub(crate) struct MultiMockStream(VecDeque<Result<StreamChunk, LoopalError>>);
impl FutStream for MultiMockStream {
    type Item = Result<StreamChunk, LoopalError>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::task::Poll::Ready(self.0.pop_front())
    }
}
impl Unpin for MultiMockStream {}

/// Provider that returns different chunks on successive calls.
pub(crate) struct MultiCallProvider {
    calls: std::sync::Mutex<VecDeque<Vec<Result<StreamChunk, LoopalError>>>>,
}
impl MultiCallProvider {
    pub(crate) fn new(calls: Vec<Vec<Result<StreamChunk, LoopalError>>>) -> Self {
        Self {
            calls: std::sync::Mutex::new(VecDeque::from(calls)),
        }
    }
}
#[async_trait]
impl Provider for MultiCallProvider {
    fn name(&self) -> &str {
        "anthropic"
    }
    async fn stream_chat(&self, _p: &ChatParams) -> Result<ChatStream, LoopalError> {
        let chunks = self.calls.lock().unwrap().pop_front().unwrap_or_default();
        Ok(Box::pin(MultiMockStream(VecDeque::from(chunks))))
    }
}

fn make_test_budget() -> ContextBudget {
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

pub(crate) fn make_multi_runner(
    calls: Vec<Vec<Result<StreamChunk, LoopalError>>>,
    _unused: bool,
) -> (AgentLoopRunner, mpsc::Receiver<loopal_protocol::AgentEvent>) {
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
    let kernel = Kernel::new(Settings::default()).unwrap();
    let mut kernel = kernel;
    kernel.register_provider(Arc::new(MultiCallProvider::new(calls)) as Arc<dyn Provider>);
    let params = AgentLoopParams {
        config: AgentConfig {
            ..Default::default()
        },
        deps: AgentDeps {
            kernel: Arc::new(kernel),
            frontend,
            session_manager: fixture.session_manager(),
        },
        session: fixture.test_session("test-multi"),
        store: ContextStore::from_messages(
            vec![loopal_message::Message::user("go")],
            make_test_budget(),
        ),
        interrupt: InterruptHandle::new(),
        shared: None,
        memory_channel: None,
        scheduled_rx: None,
        auto_classifier: None,
        harness: loopal_config::HarnessConfig::default(),
        rewake_rx: None,
        message_snapshot: None,
    };
    (AgentLoopRunner::new(params), event_rx)
}

/// LLM returns text-only response -> turn exits with Goal.
#[tokio::test]
async fn test_text_only_exits_turn() {
    use loopal_provider_api::StopReason;
    let calls = vec![vec![
        Ok(StreamChunk::Text {
            text: "all tasks done".into(),
        }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        }),
    ]];
    let (mut runner, mut event_rx) = make_multi_runner(calls, false);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.terminate_reason, loopal_error::TerminateReason::Goal);
    assert_eq!(output.result, "all tasks done");
    assert_eq!(runner.turn_count, 1);
}

/// LLM tool -> LLM text: two LLM calls inside one run.
#[tokio::test]
async fn test_tool_then_text_two_llm_calls() {
    use loopal_provider_api::StopReason;
    let tmp = std::env::temp_dir().join(format!("la_e2e_{}.txt", std::process::id()));
    std::fs::write(&tmp, "x").unwrap();
    let calls = vec![
        vec![
            Ok(StreamChunk::ToolUse {
                id: "tc-1".into(),
                name: "Read".into(),
                input: serde_json::json!({"file_path": tmp.to_str().unwrap()}),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
        vec![
            Ok(StreamChunk::Text {
                text: "read done".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls, false);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.terminate_reason, loopal_error::TerminateReason::Goal);
    assert_eq!(output.result, "read done");
    assert_eq!(runner.turn_count, 1);
    let _ = std::fs::remove_file(&tmp);
}
