use std::collections::VecDeque;
use std::sync::Arc;

use chrono::Utc;
use futures::stream::Stream as FutStream;
use loopagent_context::ContextPipeline;
use loopagent_kernel::Kernel;
use loopagent_runtime::agent_loop::AgentLoopRunner;
use loopagent_runtime::{AgentLoopParams, AgentMode, SessionManager};
use loopagent_storage::Session;
use loopagent_types::command::UserCommand;
use loopagent_types::config::Settings;
use loopagent_types::error::LoopAgentError;
use loopagent_types::event::AgentEvent;
use loopagent_types::permission::PermissionMode;
use loopagent_types::provider::{ChatParams, ChatStream, Provider, StreamChunk};
use tokio::sync::mpsc;

// --- Mock Provider for stream_llm / run() testing ---

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

pub fn make_runner_with_mock_provider(
    chunks: Vec<Result<StreamChunk, LoopAgentError>>,
) -> (AgentLoopRunner, mpsc::Receiver<AgentEvent>, mpsc::Sender<UserCommand>) {
    let (event_tx, event_rx) = mpsc::channel(64);
    let (input_tx, input_rx) = mpsc::channel(16);
    let (_perm_tx, permission_rx) = mpsc::channel(16);

    let mut kernel = Kernel::new(Settings::default()).unwrap();
    let mock = Arc::new(MockProvider::new(chunks)) as Arc<dyn Provider>;
    kernel.register_provider(mock);
    let kernel = Arc::new(kernel);

    let session = Session {
        id: "test-mock".to_string(),
        title: "".to_string(),
        model: "claude-sonnet-4-20250514".to_string(),
        cwd: "/tmp".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        mode: "default".to_string(),
    };

    let tmp_dir = std::env::temp_dir().join(format!(
        "loopagent_test_mock_{}_{}", std::process::id(), std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()
    ));
    let session_manager = SessionManager::with_base_dir(tmp_dir);

    let params = AgentLoopParams {
        kernel,
        session,
        messages: vec![loopagent_types::message::Message::user("hello")],
        model: "claude-sonnet-4-20250514".to_string(),
        system_prompt: "test".to_string(),
        mode: AgentMode::Act,
        permission_mode: PermissionMode::BypassPermissions,
        max_turns: 5,
        event_tx,
        input_rx,
        permission_rx,
        session_manager,
        context_pipeline: ContextPipeline::new(),
    };

    let runner = AgentLoopRunner::new(params);
    (runner, event_rx, input_tx)
}
