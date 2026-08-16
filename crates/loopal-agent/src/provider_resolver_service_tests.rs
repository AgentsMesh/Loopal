use std::collections::VecDeque;
use std::sync::{Arc, Mutex, RwLock};

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_provider_api::{
    ChatParams, ChatStream, Provider, SharedThinkingConfig, StopReason, StreamChunk, TaskType,
    ThinkingConfig,
};
use loopal_tool_api::{
    FetchRefinerPolicy, OneShotChatEffort, OneShotChatError, OneShotChatService,
};
use tokio_util::sync::CancellationToken;

use super::AgentShared;
use crate::shared::SchedulerHandle;
use crate::{InMemoryTaskStorage, LiveOneShotChatService, TaskStore};

enum Reply {
    Chunks(Vec<Result<StreamChunk, LoopalError>>),
    StreamError,
    Pending,
}

struct ScriptedProvider(Mutex<VecDeque<Reply>>);

#[async_trait]
impl Provider for ScriptedProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn stream_chat(&self, _: &ChatParams) -> Result<ChatStream, LoopalError> {
        match self.0.lock().unwrap().pop_front().unwrap() {
            Reply::Chunks(chunks) => Ok(Box::pin(futures::stream::iter(chunks))),
            Reply::StreamError => Err(LoopalError::Other("stream failed".into())),
            Reply::Pending => Ok(Box::pin(futures::stream::pending())),
        }
    }
}

fn shared(replies: Vec<Reply>, settings: loopal_config::Settings) -> Arc<AgentShared> {
    let mut kernel = loopal_kernel::Kernel::new(settings).unwrap();
    kernel.register_provider(Arc::new(ScriptedProvider(Mutex::new(VecDeque::from(
        replies,
    )))));
    let kernel = Arc::new(kernel);
    let (stream, _) = loopal_ipc::duplex_pair();
    let (hub_connection, _) = loopal_ipc::Connection::new(stream).into_listening();
    Arc::new(AgentShared {
        kernel,
        task_store: Arc::new(TaskStore::with_session_storage(Arc::new(
            InMemoryTaskStorage::new(),
        ))),
        hub_connection,
        cwd: ".".into(),
        depth: 0,
        agent_name: "main".into(),
        parent_event_tx: None,
        cancel_token: None,
        scheduler_handle: SchedulerHandle::new(
            Arc::new(loopal_scheduler::CronScheduler::new()),
            CancellationToken::new(),
        ),
        message_snapshot: Arc::new(RwLock::new(Vec::new())),
        goal_session: None,
        workflow_control: None,
    })
}

fn text(value: &str) -> Result<StreamChunk, LoopalError> {
    Ok(StreamChunk::Text { text: value.into() })
}

fn done() -> Result<StreamChunk, LoopalError> {
    Ok(StreamChunk::Done {
        stop_reason: StopReason::EndTurn,
    })
}

#[tokio::test]
async fn one_shot_stream_contract_covers_success_and_failures() {
    let agent = shared(
        vec![
            Reply::Chunks(vec![
                text("answer"),
                Ok(StreamChunk::Thinking {
                    text: "ignored".into(),
                }),
                done(),
            ]),
            Reply::StreamError,
            Reply::Chunks(vec![Err(LoopalError::Other("chunk failed".into()))]),
            Reply::Chunks(vec![done()]),
        ],
        loopal_config::Settings::default(),
    );

    assert_eq!(
        agent
            .one_shot_chat("claude-sonnet-4-6", "s", "u", 32)
            .await
            .unwrap(),
        "answer"
    );
    assert_eq!(
        agent
            .one_shot_chat_with_effort("claude-sonnet-4-6", "s", "u", 32, OneShotChatEffort::Max,)
            .await,
        Err(OneShotChatError::StreamFailed)
    );
    assert_eq!(
        agent.one_shot_chat("claude-sonnet-4-6", "s", "u", 32).await,
        Err(OneShotChatError::ChunkFailed)
    );
    assert_eq!(
        agent.one_shot_chat("claude-sonnet-4-6", "s", "u", 32).await,
        Err(OneShotChatError::EmptyResponse)
    );
}

#[tokio::test(start_paused = true)]
async fn one_shot_timeout_and_live_thinking_reader_are_honored() {
    let timed_out = shared(vec![Reply::Pending], loopal_config::Settings::default());
    assert_eq!(
        timed_out
            .one_shot_chat("claude-sonnet-4-6", "s", "u", 32)
            .await,
        Err(OneShotChatError::Timeout)
    );

    let agent = shared(
        vec![
            Reply::Chunks(vec![text("first"), done()]),
            Reply::Chunks(vec![text("second")]),
        ],
        loopal_config::Settings::default(),
    );
    let thinking = SharedThinkingConfig::new(ThinkingConfig::Auto);
    let service = LiveOneShotChatService::new(agent, thinking.reader());
    assert_eq!(
        service
            .one_shot_chat("claude-sonnet-4-6", "s", "u", 32)
            .await
            .unwrap(),
        "first"
    );
    thinking.set(ThinkingConfig::Disabled);
    assert_eq!(
        service
            .one_shot_chat_with_effort("claude-sonnet-4-6", "s", "u", 32, OneShotChatEffort::Max,)
            .await
            .unwrap(),
        "second"
    );
}

#[tokio::test]
async fn fetch_refiner_requires_enabled_large_body_and_route() {
    let mut settings = loopal_config::Settings::default();
    settings.fetch_refiner.threshold_bytes = 10;
    settings
        .model_routing
        .insert(TaskType::Refine, "refiner".into());
    let agent = shared(Vec::new(), settings);
    assert_eq!(agent.refiner_model(10), None);
    assert_eq!(agent.refiner_model(11).as_deref(), Some("refiner"));

    let mut disabled = loopal_config::Settings::default();
    disabled.fetch_refiner.enabled = false;
    assert_eq!(shared(Vec::new(), disabled).refiner_model(usize::MAX), None);
}
