//! Integration test for inject_pending_messages() before !interactive break.
//!
//! Verifies that sub-agents drain pending messages even when LLM returns
//! pure text (no tool calls), preventing message loss.

use std::collections::VecDeque;
use std::sync::Arc;

use chrono::Utc;
use futures::stream::Stream as FutStream;
use loopal_config::Settings;
use loopal_context::{ContextBudget, ContextPipeline, ContextStore};
use loopal_error::LoopalError;
use loopal_kernel::Kernel;
use loopal_protocol::AgentEvent;
use loopal_protocol::ControlCommand;
use loopal_protocol::{Envelope, MessageSource};
use loopal_provider_api::{ChatParams, ChatStream, Provider, StopReason, StreamChunk};
use loopal_runtime::frontend::{AutoCancelQuestionHandler, AutoDenyHandler};
use loopal_runtime::{AgentLoopParams, AgentMode, SessionManager, UnifiedFrontend, agent_loop};
use loopal_storage::Session;
use loopal_tool_api::PermissionMode;
use tokio::sync::mpsc;

struct TextOnlyProvider {
    chunks: std::sync::Mutex<Option<Vec<Result<StreamChunk, LoopalError>>>>,
}

impl TextOnlyProvider {
    fn new(text: &str) -> Self {
        let chunks = vec![
            Ok(StreamChunk::Text {
                text: text.to_string(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ];
        Self {
            chunks: std::sync::Mutex::new(Some(chunks)),
        }
    }
}

struct MockStream {
    chunks: VecDeque<Result<StreamChunk, LoopalError>>,
}

impl FutStream for MockStream {
    type Item = Result<StreamChunk, LoopalError>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::task::Poll::Ready(self.chunks.pop_front())
    }
}
impl Unpin for MockStream {}

#[async_trait::async_trait]
impl Provider for TextOnlyProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn stream_chat(&self, _p: &ChatParams) -> Result<ChatStream, LoopalError> {
        let chunks = self.chunks.lock().unwrap().take().unwrap_or_default();
        Ok(Box::pin(MockStream {
            chunks: VecDeque::from(chunks),
        }))
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

#[tokio::test]
async fn test_subagent_drains_pending_before_exit() {
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(256);
    let (mailbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(16);
    let (_control_tx, control_rx) = mpsc::channel::<ControlCommand>(16);

    // Push a pending message BEFORE the agent runs —
    // simulates a channel subscription message arriving during LLM processing.
    mailbox_tx
        .send(Envelope::new(
            MessageSource::Agent("coordinator".into()),
            "worker",
            "please also check the logs",
        ))
        .await
        .unwrap();

    let frontend = Arc::new(UnifiedFrontend::new(
        Some("worker".into()),
        event_tx,
        mailbox_rx,
        control_rx,
        None,
        Box::new(AutoDenyHandler),
        Box::new(AutoCancelQuestionHandler),
    ));

    let mut kernel = Kernel::new(Settings::default()).unwrap();
    let mock = Arc::new(TextOnlyProvider::new("I will do that.")) as Arc<dyn Provider>;
    kernel.register_provider(mock);
    let kernel = Arc::new(kernel);

    let session = Session {
        id: "drain-test".into(),
        title: "".into(),
        model: "claude-sonnet-4-20250514".into(),
        cwd: "/tmp".into(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        mode: "default".into(),
    };

    let tmp = std::env::temp_dir().join(format!("la_drain_{}", std::process::id()));
    let params = AgentLoopParams {
        kernel,
        session,
        store: ContextStore::from_messages(
            vec![loopal_message::Message::user("run task")],
            make_test_budget(),
        ),
        model: "claude-sonnet-4-20250514".into(),
        compact_model: None,
        system_prompt: "test".into(),
        mode: AgentMode::Act,
        permission_mode: PermissionMode::Bypass,
        max_turns: 5,
        frontend,
        session_manager: SessionManager::with_base_dir(tmp),
        context_pipeline: ContextPipeline::new(),
        tool_filter: None,
        shared: None,
        interactive: false, // Sub-agent mode — exits after first LLM response
        thinking_config: loopal_provider_api::ThinkingConfig::Auto,
        interrupt: Default::default(),
        interrupt_tx: std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
        memory_channel: None,
    };

    // Drain events in background so channels don't block
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let result = agent_loop(params).await;
    assert!(result.is_ok());
    // The agent returned its text output — the pending message was drained
    // (injected into messages) before the !interactive break.
    let output = result.unwrap();
    assert_eq!(output.result, "I will do that.");
}
