mod message_builder;
mod stream;
mod thinking;

use async_trait::async_trait;
use loopal_error::{LoopalError, ProviderError};
use loopal_provider_api::{ChatParams, ChatStream, Provider};
use reqwest::Client;
use serde_json::json;
use std::collections::VecDeque;
use std::time::Duration;

use crate::sse::SseStream;
use stream::ToolCallAccumulator;

pub struct OpenAiProvider {
    client: Client,
    api_key: String,
    base_url: String,
}

impl OpenAiProvider {
    pub fn new(api_key: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(300))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("failed to build HTTP client");
        Self {
            client,
            api_key,
            base_url: "https://api.openai.com".to_string(),
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }
}

#[async_trait]
impl Provider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    async fn stream_chat(&self, params: &ChatParams) -> Result<ChatStream, LoopalError> {
        let messages = self.build_messages(params);
        let tools = self.build_tools(params);

        let mut body = json!({
            "model": params.model,
            "stream": true,
            "messages": messages,
            "max_completion_tokens": params.max_tokens,
        });

        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }
        if let Some(temp) = params.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(ref thinking_config) = params.thinking {
            body["reasoning_effort"] = json!(thinking::to_openai_reasoning_effort(thinking_config));
            // Reasoning models don't support temperature
            body.as_object_mut().unwrap().remove("temperature");
        }

        // Add stream_options to get usage in stream
        body["stream_options"] = json!({"include_usage": true});

        tracing::info!(
            model = %params.model,
            url = %format!("{}/v1/chat/completions", self.base_url),
            messages = params.messages.len(),
            tools = params.tools.len(),
            max_tokens = params.max_tokens,
            "API request"
        );

        let response = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let status = response.status();
        tracing::info!(status = status.as_u16(), "API response");
        if !status.is_success() {
            // Detect rate limiting (429)
            if status.as_u16() == 429 {
                let retry_after_ms = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<f64>().ok())
                    .map(|secs| (secs * 1000.0) as u64)
                    .unwrap_or(30_000);
                tracing::warn!(retry_after_ms, "rate limited by API");
                return Err(ProviderError::RateLimited { retry_after_ms }.into());
            }
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "failed to read body".into());
            tracing::error!(status = status.as_u16(), body = %text, "API error");
            return Err(ProviderError::Api {
                status: status.as_u16(),
                message: text,
            }
            .into());
        }

        let sse = SseStream::from_response(response);
        let stream = stream::OpenAiStream {
            inner: Box::pin(sse),
            state: ToolCallAccumulator::default(),
            buffer: VecDeque::new(),
        };
        Ok(Box::pin(stream))
    }
}
