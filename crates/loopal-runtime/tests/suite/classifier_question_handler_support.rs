use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use loopal_error::{LoopalError, Result as LResult};
use loopal_protocol::{AgentEventPayload, Question, UserQuestionResponse};
use loopal_provider_api::{
    ChatParams, ChatStream, Provider, ProviderResolver, StopReason, StreamChunk, TaskType,
};
use loopal_runtime::frontend::question_handler::{AskOptions, QuestionHandler, QuestionOutcome};
use loopal_runtime::frontend::traits::EventEmitter;

pub struct FailingResolver;

impl ProviderResolver for FailingResolver {
    fn resolve_for(
        &self,
        _task: TaskType,
    ) -> std::result::Result<(String, Arc<dyn Provider>), LoopalError> {
        Err(LoopalError::Other("resolver failure".into()))
    }
}

pub struct StubResolver {
    pub provider: Arc<dyn Provider>,
    pub model: String,
}

impl ProviderResolver for StubResolver {
    fn resolve_for(
        &self,
        _task: TaskType,
    ) -> std::result::Result<(String, Arc<dyn Provider>), LoopalError> {
        Ok((self.model.clone(), self.provider.clone()))
    }
}

pub struct ScriptedProvider {
    response: Mutex<Option<String>>,
    delay: Duration,
}

impl ScriptedProvider {
    pub fn returning(json: &str) -> Arc<Self> {
        Arc::new(Self {
            response: Mutex::new(Some(json.to_string())),
            delay: Duration::ZERO,
        })
    }

    pub fn returning_after(json: &str, delay: Duration) -> Arc<Self> {
        Arc::new(Self {
            response: Mutex::new(Some(json.to_string())),
            delay,
        })
    }
}

struct ScriptedStream(VecDeque<std::result::Result<StreamChunk, LoopalError>>);
impl futures::Stream for ScriptedStream {
    type Item = std::result::Result<StreamChunk, LoopalError>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::task::Poll::Ready(self.0.pop_front())
    }
}
impl Unpin for ScriptedStream {}

#[async_trait]
impl Provider for ScriptedProvider {
    fn name(&self) -> &str {
        "scripted"
    }
    async fn stream_chat(&self, _p: &ChatParams) -> std::result::Result<ChatStream, LoopalError> {
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        let text = self.response.lock().unwrap().take().unwrap_or_default();
        let chunks = VecDeque::from(vec![
            Ok(StreamChunk::Text { text }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ]);
        Ok(Box::pin(ScriptedStream(chunks)))
    }
}

pub struct DelayedFallback {
    pub delay: Duration,
    pub answer: UserQuestionResponse,
    pub call_count: Arc<AtomicUsize>,
    pub last_options: Arc<Mutex<Option<AskOptions>>>,
}

impl DelayedFallback {
    pub fn new(delay: Duration, answer: UserQuestionResponse) -> Self {
        Self {
            delay,
            answer,
            call_count: Arc::new(AtomicUsize::new(0)),
            last_options: Arc::new(Mutex::new(None)),
        }
    }
}

#[async_trait]
impl QuestionHandler for DelayedFallback {
    async fn ask(&self, _q: Vec<Question>) -> QuestionOutcome {
        self.ask_with_options(_q, AskOptions::manual("")).await
    }

    async fn ask_with_options(&self, _q: Vec<Question>, options: AskOptions) -> QuestionOutcome {
        *self.last_options.lock().unwrap() = Some(options);
        tokio::time::sleep(self.delay).await;
        self.call_count.fetch_add(1, Ordering::SeqCst);
        QuestionOutcome::manual(self.answer.clone())
    }
}

#[derive(Default, Clone)]
pub struct RecordingEmitter {
    pub events: Arc<Mutex<Vec<AgentEventPayload>>>,
}

impl RecordingEmitter {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn count_kind(&self, predicate: impl Fn(&AgentEventPayload) -> bool) -> usize {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|p| predicate(p))
            .count()
    }
}

#[async_trait]
impl EventEmitter for RecordingEmitter {
    async fn emit(&self, payload: AgentEventPayload) -> LResult<()> {
        self.events.lock().unwrap().push(payload);
        Ok(())
    }
}
