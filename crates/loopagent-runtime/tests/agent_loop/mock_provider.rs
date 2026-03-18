use std::collections::VecDeque;
use std::sync::Arc;

use chrono::Utc;
use futures::stream::Stream as FutStream;
use loopagent_context::ContextPipeline;
use loopagent_kernel::Kernel;
use loopagent_runtime::agent_loop::AgentLoopRunner;
use loopagent_runtime::frontend::AutoDenyHandler;
use loopagent_runtime::{AgentLoopParams, AgentMode, SessionManager, UnifiedFrontend};
use loopagent_storage::Session;
use loopagent_types::config::Settings;
use loopagent_types::control::ControlCommand;
use loopagent_types::envelope::Envelope;
use loopagent_types::error::LoopAgentError;
use loopagent_types::event::AgentEvent;
use loopagent_types::permission::PermissionMode;
use loopagent_types::provider::{ChatParams, ChatStream, Provider, StreamChunk};
use tokio::sync::mpsc;

pub struct MockStreamChunks {
    pub chunks: VecDeque<Result<StreamChunk, LoopAgentError>>,
}

impl FutStream for MockStreamChunks {
    type Item = Result<StreamChunk, LoopAgentError>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::task::Poll::Ready(self.chunks.pop_front())
    }
}

impl Unpin for MockStreamChunks {}

pub struct MockProvider {
    pub chunks: std::sync::Mutex<Option<Vec<Result<StreamChunk, LoopAgentError>>>>,
}

impl MockProvider {
    pub fn new(chunks: Vec<Result<StreamChunk, LoopAgentError>>) -> Self {
        Self {
            chunks: std::sync::Mutex::new(Some(chunks)),
        }
    }
}

#[async_trait::async_trait]
impl Provider for MockProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn stream_chat(&self, _params: &ChatParams) -> Result<ChatStream, LoopAgentError> {
        let chunks = self.chunks.lock().unwrap().take().unwrap_or_default();
        let stream = MockStreamChunks {
            chunks: VecDeque::from(chunks),
        };
        Ok(Box::pin(stream))
    }
}

fn test_session(id: &str) -> Session {
    Session {
        id: id.into(), title: "".into(),
        model: "claude-sonnet-4-20250514".into(), cwd: "/tmp".into(),
        created_at: Utc::now(), updated_at: Utc::now(), mode: "default".into(),
    }
}

pub fn make_runner_with_mock_provider(
    chunks: Vec<Result<StreamChunk, LoopAgentError>>,
) -> (AgentLoopRunner, mpsc::Receiver<AgentEvent>, mpsc::Sender<Envelope>, mpsc::Sender<ControlCommand>) {
    let (event_tx, event_rx) = mpsc::channel(64);
    let (mbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(16);
    let (ctrl_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let frontend = Arc::new(UnifiedFrontend::new(
        None, event_tx, mailbox_rx, control_rx, None, Box::new(AutoDenyHandler),
    ));
    let mut kernel = Kernel::new(Settings::default()).unwrap();
    kernel.register_provider(Arc::new(MockProvider::new(chunks)) as Arc<dyn Provider>);
    let tmp = std::env::temp_dir().join(format!(
        "loopagent_test_mock_{}_{}", std::process::id(), std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()
    ));
    let params = AgentLoopParams {
        kernel: Arc::new(kernel), session: test_session("test-mock"),
        messages: vec![loopagent_types::message::Message::user("hello")],
        model: "claude-sonnet-4-20250514".into(), system_prompt: "test".into(),
        mode: AgentMode::Act, permission_mode: PermissionMode::Bypass,
        max_turns: 5, frontend, session_manager: SessionManager::with_base_dir(tmp),
        context_pipeline: ContextPipeline::new(),
        tool_filter: None, shared: None, interactive: true,
    };
    (AgentLoopRunner::new(params), event_rx, mbox_tx, ctrl_tx)
}

pub struct MultiCallProvider {
    pub calls: std::sync::Mutex<VecDeque<Vec<Result<StreamChunk, LoopAgentError>>>>,
}
impl MultiCallProvider {
    pub fn new(calls: Vec<Vec<Result<StreamChunk, LoopAgentError>>>) -> Self {
        Self { calls: std::sync::Mutex::new(VecDeque::from(calls)) }
    }
}
#[async_trait::async_trait]
impl Provider for MultiCallProvider {
    fn name(&self) -> &str { "anthropic" }
    async fn stream_chat(&self, _p: &ChatParams) -> Result<ChatStream, LoopAgentError> {
        let chunks = self.calls.lock().unwrap().pop_front().unwrap_or_default();
        Ok(Box::pin(MockStreamChunks { chunks: VecDeque::from(chunks) }))
    }
}

/// Create a non-interactive runner backed by a MultiCallProvider.
pub fn make_multi_runner(
    calls: Vec<Vec<Result<StreamChunk, LoopAgentError>>>,
) -> (AgentLoopRunner, mpsc::Receiver<AgentEvent>) {
    let (event_tx, event_rx) = mpsc::channel(64);
    let (_mbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(16);
    let (_ctrl_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let frontend = Arc::new(UnifiedFrontend::new(
        None, event_tx, mailbox_rx, control_rx, None, Box::new(AutoDenyHandler),
    ));
    let mut kernel = Kernel::new(Settings::default()).unwrap();
    kernel.register_provider(Arc::new(MultiCallProvider::new(calls)) as Arc<dyn Provider>);
    let tmp = std::env::temp_dir().join(format!("la_multi_{}", std::process::id()));
    let params = AgentLoopParams {
        kernel: Arc::new(kernel), session: test_session("test-multi"),
        messages: vec![loopagent_types::message::Message::user("go")],
        model: "claude-sonnet-4-20250514".into(), system_prompt: "t".into(),
        mode: AgentMode::Act, permission_mode: PermissionMode::Bypass,
        max_turns: 10, frontend, session_manager: SessionManager::with_base_dir(tmp),
        context_pipeline: ContextPipeline::new(),
        tool_filter: None, shared: None, interactive: false,
    };
    (AgentLoopRunner::new(params), event_rx)
}

/// Create an interactive runner with MultiCallProvider and custom kernel setup.
/// Returns senders for test-controlled input injection.
pub fn make_interactive_multi_runner(
    calls: Vec<Vec<Result<StreamChunk, LoopAgentError>>>,
    setup: impl FnOnce(&mut Kernel),
) -> (AgentLoopRunner, mpsc::Receiver<AgentEvent>, mpsc::Sender<Envelope>, mpsc::Sender<ControlCommand>) {
    let (event_tx, event_rx) = mpsc::channel(64);
    let (mbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(16);
    let (ctrl_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let frontend = Arc::new(UnifiedFrontend::new(
        None, event_tx, mailbox_rx, control_rx, None, Box::new(AutoDenyHandler),
    ));
    let mut kernel = Kernel::new(Settings::default()).unwrap();
    kernel.register_provider(Arc::new(MultiCallProvider::new(calls)) as Arc<dyn Provider>);
    setup(&mut kernel);
    let tmp = std::env::temp_dir().join(format!("la_int_{}", std::process::id()));
    let params = AgentLoopParams {
        kernel: Arc::new(kernel), session: test_session("test-interactive"),
        messages: vec![loopagent_types::message::Message::user("go")],
        model: "claude-sonnet-4-20250514".into(), system_prompt: "t".into(),
        mode: AgentMode::Act, permission_mode: PermissionMode::Bypass,
        max_turns: 10, frontend, session_manager: SessionManager::with_base_dir(tmp),
        context_pipeline: ContextPipeline::new(),
        tool_filter: None, shared: None, interactive: true,
    };
    (AgentLoopRunner::new(params), event_rx, mbox_tx, ctrl_tx)
}
