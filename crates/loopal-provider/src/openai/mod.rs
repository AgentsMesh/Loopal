mod input_builder;
pub(crate) mod server_tool;
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
        let input = self.build_input(params);
        let tools = self.build_tools(params);

        let mut body = json!({
            "model": params.model,
            "stream": true,
            "input": input,
            "max_output_tokens": params.max_tokens,
        });

        if !params.system_prompt.is_empty() {
            body["instructions"] = json!(params.system_prompt);
        }
        if !tools.is_empty() {
            body["tools"] = json!(tools);
            body["tool_choice"] = json!("auto");
        }
        if let Some(ref thinking_config) = params.thinking {
            body["reasoning"] = thinking::to_openai_reasoning(thinking_config);
        } else if let Some(temp) = params.temperature {
            body["temperature"] = json!(temp);
        }

        tracing::info!(
            model = %params.model,
            url = %format!("{}/v1/responses", self.base_url),
            messages = params.messages.len(),
            tools = tools.len(),
            max_tokens = params.max_tokens,
            "API request"
        );

        let response = self
            .client
            .post(format!("{}/v1/responses", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let status = response.status();
        tracing::info!(status = status.as_u16(), "API response");
        if !status.is_success() {
            return Err(self.handle_error_response(response, status).await);
        }

        let sse = SseStream::from_response(response);
        let stream = stream::OpenAiStream {
            inner: Box::pin(sse),
            buffer: VecDeque::new(),
        };
        Ok(Box::pin(stream))
    }
}

impl OpenAiProvider {
    async fn handle_error_response(
        &self,
        response: reqwest::Response,
        status: reqwest::StatusCode,
    ) -> LoopalError {
        if status.as_u16() == 429 {
            let retry_after_ms = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<f64>().ok())
                .map(|secs| (secs * 1000.0) as u64)
                .unwrap_or(30_000);
            tracing::warn!(retry_after_ms, "rate limited by API");
            return ProviderError::RateLimited { retry_after_ms }.into();
        }
        let text = response
            .text()
            .await
            .unwrap_or_else(|_| "failed to read body".into());
        tracing::error!(status = status.as_u16(), body = %text, "API error");
        ProviderError::Api {
            status: status.as_u16(),
            message: text,
        }
        .into()
    }
}
