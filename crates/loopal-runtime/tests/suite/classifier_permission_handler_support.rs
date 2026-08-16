use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_protocol::PermissionIntentRequest;
use loopal_provider_api::{
    ChatParams, ChatStream, Provider, ProviderResolver, StopReason, StreamChunk, TaskType,
};
use loopal_runtime::frontend::permission_handler::{PermissionHandler, PermissionOutcome};
use loopal_tool_api::PermissionDecision;

pub struct FailingResolver;

impl ProviderResolver for FailingResolver {
    fn resolve_for(
        &self,
        _task: TaskType,
    ) -> std::result::Result<(String, Arc<dyn Provider>), LoopalError> {
        Err(LoopalError::Other("test resolver failure".into()))
    }
}

pub struct MockResolver {
    pub provider: Arc<dyn Provider>,
    pub model: String,
}

impl ProviderResolver for MockResolver {
    fn resolve_for(
        &self,
        _task: TaskType,
    ) -> std::result::Result<(String, Arc<dyn Provider>), LoopalError> {
        Ok((self.model.clone(), self.provider.clone()))
    }
}

pub struct MockProvider {
    response: std::sync::Mutex<Option<String>>,
}

impl MockProvider {
    pub fn returning(json: &str) -> Arc<Self> {
        Arc::new(Self {
            response: std::sync::Mutex::new(Some(json.to_string())),
        })
    }
}

struct MockStream(VecDeque<std::result::Result<StreamChunk, LoopalError>>);
impl futures::Stream for MockStream {
    type Item = std::result::Result<StreamChunk, LoopalError>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::task::Poll::Ready(self.0.pop_front())
    }
}
impl Unpin for MockStream {}

#[async_trait]
impl Provider for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }
    async fn stream_chat(&self, _p: &ChatParams) -> std::result::Result<ChatStream, LoopalError> {
        let text = self.response.lock().unwrap().take().unwrap();
        let chunks = VecDeque::from(vec![
            Ok(StreamChunk::Text { text }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ]);
        Ok(Box::pin(MockStream(chunks)))
    }
}

pub struct RecordingHandler {
    pub called: Arc<AtomicBool>,
    pub decision: PermissionDecision,
}

#[async_trait]
impl PermissionHandler for RecordingHandler {
    async fn decide(&self, _request: &PermissionIntentRequest) -> PermissionOutcome {
        self.called.store(true, Ordering::SeqCst);
        PermissionOutcome {
            decision: self.decision,
            reason: "mock".into(),
            duration_ms: 0,
            receipt: None,
        }
    }
}
