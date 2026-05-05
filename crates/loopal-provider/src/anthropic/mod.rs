mod accumulator;
mod error_classify;
mod finalize;
mod request;
mod send;
pub(crate) mod server_tool;
mod stream;
mod stream_parser;
mod thinking;

use std::borrow::Cow;
use std::time::Duration;

use async_trait::async_trait;
use loopal_error::{LoopalError, ProviderError};
use loopal_message::Message;
use loopal_provider_api::{ChatParams, ChatStream, ErrorClass, Provider};
use tokio::sync::Semaphore;

use crate::resilient_client::ResilientClient;

const MAX_CONCURRENT_REQUESTS: usize = 3;

pub struct AnthropicProvider {
    pub(super) client: ResilientClient,
    pub(super) api_key: String,
    pub(super) base_url: String,
    request_semaphore: Semaphore,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: ResilientClient::new(Duration::from_secs(300), Duration::from_secs(10)),
            api_key,
            base_url: "https://api.anthropic.com".to_string(),
            request_semaphore: Semaphore::new(MAX_CONCURRENT_REQUESTS),
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn stream_chat(&self, params: &ChatParams) -> Result<ChatStream, LoopalError> {
        // Gate the initial HTTP request only — SSE stream lifetime continues
        // after permit is dropped.
        let _permit = self
            .request_semaphore
            .acquire()
            .await
            .map_err(|_| ProviderError::Http("request semaphore closed".into()))?;
        self.do_stream_chat(params).await
    }

    fn finalize_messages<'a>(&self, params: &'a ChatParams) -> Cow<'a, [Message]> {
        self.finalize_messages_impl(params)
    }

    fn classify_error(&self, err: &LoopalError) -> ErrorClass {
        self.classify_error_impl(err)
    }
}
