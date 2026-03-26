//! Provider wrapper that captures every `ChatParams` for test assertions.

use std::sync::{Arc, Mutex};

use loopal_error::LoopalError;
use loopal_message::Message;
use loopal_provider_api::{ChatParams, ChatStream, Provider};

/// Snapshot of a single `stream_chat` invocation.
#[derive(Debug, Clone)]
pub struct CapturedRequest {
    pub messages: Vec<Message>,
    pub system_prompt: String,
    pub model: String,
}

/// Wraps any `Provider` and records each `stream_chat` call's parameters.
pub struct CapturedProvider {
    inner: Arc<dyn Provider>,
    requests: Mutex<Vec<CapturedRequest>>,
}

impl CapturedProvider {
    pub fn wrapping(inner: Arc<dyn Provider>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            requests: Mutex::new(Vec::new()),
        })
    }

    /// Return all captured requests so far.
    pub fn captured(&self) -> Vec<CapturedRequest> {
        self.requests.lock().unwrap().clone()
    }

    /// Return the last captured request, if any.
    pub fn last_request(&self) -> Option<CapturedRequest> {
        self.requests.lock().unwrap().last().cloned()
    }
}

#[async_trait::async_trait]
impl Provider for CapturedProvider {
    fn name(&self) -> &str {
        self.inner.name()
    }

    async fn stream_chat(&self, params: &ChatParams) -> Result<ChatStream, LoopalError> {
        self.requests.lock().unwrap().push(CapturedRequest {
            messages: params.messages.clone(),
            system_prompt: params.system_prompt.clone(),
            model: params.model.clone(),
        });
        self.inner.stream_chat(params).await
    }
}
