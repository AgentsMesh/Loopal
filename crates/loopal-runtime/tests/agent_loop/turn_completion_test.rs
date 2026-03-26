//! Tests for AttemptCompletion end-to-end flow through execute_turn,
//! and additional turn-level edge cases.

use std::collections::VecDeque;
use std::sync::Arc;

use async_trait::async_trait;
use futures::stream::Stream as FutStream;
use loopal_config::Settings;
use loopal_context::{ContextBudget, ContextStore};
use loopal_error::{LoopalError, TerminateReason};
use loopal_kernel::Kernel;
use loopal_protocol::ControlCommand;
use loopal_protocol::Envelope;
use loopal_provider_api::{ChatParams, ChatStream, Provider, StopReason, StreamChunk};
use loopal_runtime::agent_loop::AgentLoopRunner;
use loopal_runtime::frontend::{AutoCancelQuestionHandler, AutoDenyHandler};
use loopal_runtime::{AgentConfig, AgentDeps, AgentLoopParams, InterruptHandle, UnifiedFrontend};
use loopal_test_support::TestFixture;
use loopal_tool_api::PermissionLevel;
use loopal_tool_api::{COMPLETION_PREFIX, Tool, ToolContext, ToolResult};
use tokio::sync::mpsc;

// --- Multi-call mock provider ---

struct MultiMockStream(VecDeque<Result<StreamChunk, LoopalError>>);
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
struct MultiCallProvider {
    calls: std::sync::Mutex<VecDeque<Vec<Result<StreamChunk, LoopalError>>>>,
}
impl MultiCallProvider {
    fn new(calls: Vec<Vec<Result<StreamChunk, LoopalError>>>) -> Self {
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

struct FakeCompletionTool;
#[async_trait]
impl Tool for FakeCompletionTool {
    fn name(&self) -> &str {
        "AttemptCompletion"
    }
    fn description(&self) -> &str {
        "test"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }
    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    async fn execute(
        &self,
        input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let r = input
            .get("result")
            .and_then(|v| v.as_str())
            .unwrap_or("done");
        Ok(ToolResult::success(format!("{COMPLETION_PREFIX}{r}")))
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
    }
}

fn make_multi_runner(
    calls: Vec<Vec<Result<StreamChunk, LoopalError>>>,
    register_completion: bool,
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
    let mut kernel = Kernel::new(Settings::default()).unwrap();
    kernel.register_provider(Arc::new(MultiCallProvider::new(calls)) as Arc<dyn Provider>);
    if register_completion {
        kernel.register_tool(Box::new(FakeCompletionTool));
    }
    let params = AgentLoopParams {
        config: AgentConfig {
            max_turns: 10,
            interactive: false,
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
    };
    (AgentLoopRunner::new(params), event_rx)
}

/// LLM returns AttemptCompletion → turn exits immediately with completed=true.
#[tokio::test]
async fn test_attempt_completion_exits_turn_immediately() {
    let calls = vec![vec![
        Ok(StreamChunk::ToolUse {
            id: "tc-1".into(),
            name: "AttemptCompletion".into(),
            input: serde_json::json!({"result": "all tasks done"}),
        }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        }),
    ]];
    let (mut runner, mut event_rx) = make_multi_runner(calls, true);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
    assert_eq!(output.result, "all tasks done");
    // Tool execution no longer increments turn_count (only user messages do)
    assert_eq!(runner.turn_count, 0);
}

/// LLM tool → LLM AttemptCompletion: two LLM calls inside one turn.
#[tokio::test]
async fn test_tool_then_completion_two_llm_calls() {
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
            Ok(StreamChunk::ToolUse {
                id: "tc-2".into(),
                name: "AttemptCompletion".into(),
                input: serde_json::json!({"result": "read done"}),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls, true);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
    assert_eq!(output.result, "read done");
    // Tool execution no longer increments turn_count (only user messages do)
    assert_eq!(runner.turn_count, 0);
    let _ = std::fs::remove_file(&tmp);
}

/// First turn succeeds, second turn errors → result preserves first output.
#[tokio::test]
async fn test_error_preserves_prior_output() {
    let calls = vec![
        vec![
            Ok(StreamChunk::Text {
                text: "first output".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
        // Second LLM call is attempted on next iteration but won't happen
        // because non-interactive exits after first turn with no tool calls.
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls, false);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.result, "first output");
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
}

/// Tool execution no longer increments turn_count, so max_turns is not hit
/// inside execute_turn. The non-interactive agent exits after the turn completes.
#[tokio::test]
async fn test_max_turns_inside_execute_turn() {
    let tmp = std::env::temp_dir().join(format!("la_mt_{}.txt", std::process::id()));
    std::fs::write(&tmp, "y").unwrap();
    let calls = vec![vec![
        Ok(StreamChunk::ToolUse {
            id: "tc-1".into(),
            name: "Read".into(),
            input: serde_json::json!({"file_path": tmp.to_str().unwrap()}),
        }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        }),
    ]];
    let (mut runner, mut event_rx) = make_multi_runner(calls, false);
    runner.params.config.max_turns = 1;
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    // Non-interactive: exits after first turn completes
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
    assert_eq!(runner.turn_count, 0);
    let _ = std::fs::remove_file(&tmp);
}

/// Regression test: tool call with text → next LLM call stream error →
/// output preserves the text from the successful iteration (not empty).
/// This was the root cause of sub-agents returning empty results.
#[tokio::test]
async fn test_stream_error_after_tool_preserves_last_text() {
    let tmp = std::env::temp_dir().join(format!("la_se_{}.txt", std::process::id()));
    std::fs::write(&tmp, "data").unwrap();
    let calls = vec![
        // First LLM call: text + tool
        vec![
            Ok(StreamChunk::Text {
                text: "I will read the file.".into(),
            }),
            Ok(StreamChunk::ToolUse {
                id: "tc-1".into(),
                name: "Read".into(),
                input: serde_json::json!({"file_path": tmp.to_str().unwrap()}),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
        // Second LLM call: stream error (simulates 502/connection reset)
        vec![Err(LoopalError::Provider(
            loopal_error::ProviderError::StreamEnded,
        ))],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls, false);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    // The key assertion: even though the second LLM call had a stream error,
    // the output preserves "I will read the file." from the first iteration.
    assert_eq!(output.result, "I will read the file.");
    assert_eq!(output.terminate_reason, TerminateReason::Goal);

    let _ = std::fs::remove_file(&tmp);
}
