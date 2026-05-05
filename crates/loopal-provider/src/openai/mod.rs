mod input_builder;
pub(crate) mod server_tool;
mod stream;
mod thinking;

use async_trait::async_trait;
use loopal_error::{LoopalError, ProviderError};
use loopal_provider_api::{ChatParams, ChatStream, ErrorClass, Provider, default_classify_error};
use serde_json::json;
use std::collections::VecDeque;
use std::time::Duration;
use tracing::Instrument;

use crate::resilient_client::ResilientClient;
use crate::sse::SseStream;

pub struct OpenAiProvider {
    client: ResilientClient,
    api_key: String,
    base_url: String,
}

impl OpenAiProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: ResilientClient::new(Duration::from_secs(300), Duration::from_secs(10)),
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
        let finalized = self.finalize_messages(params).into_owned();
        let final_params = ChatParams {
            messages: finalized,
            ..params.clone()
        };
        let input = self.build_input(&final_params);
        let tools = self.build_tools(&final_params);

        let mut body = json!({
            "model": final_params.model,
            "stream": true,
            "input": input,
            "max_output_tokens": final_params.max_tokens,
        });

        if !final_params.system_prompt.is_empty() {
            body["instructions"] = json!(final_params.system_prompt);
        }
        if !tools.is_empty() {
            body["tools"] = json!(tools);
            body["tool_choice"] = json!("auto");
        }
        if let Some(ref thinking_config) = final_params.thinking {
            body["reasoning"] = thinking::to_openai_reasoning(thinking_config);
        } else if let Some(temp) = final_params.temperature {
            body["temperature"] = json!(temp);
        }

        tracing::info!(
            model = %final_params.model,
            url = %format!("{}/v1/responses", self.base_url),
            messages = final_params.messages.len(),
            tools = tools.len(),
            max_tokens = final_params.max_tokens,
            "API request"
        );

        let http_span = tracing::info_span!("http_request", gen_ai.system = "openai");
        let (client, client_gen) = self.client.get();
        let response = client
            .post(format!("{}/v1/responses", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .instrument(http_span)
            .await
            .map_err(|e| {
                self.client.report_network_error(client_gen);
                ProviderError::Http(format!("{e:#}"))
            })?;
        self.client.report_success(client_gen);

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

    fn classify_error(&self, err: &LoopalError) -> ErrorClass {
        if let LoopalError::Provider(ProviderError::Api {
            status: 400,
            message,
        }) = err
            && is_openai_context_overflow_keyword(message)
        {
            return ErrorClass::ContextOverflow;
        }
        default_classify_error(err)
    }
}

fn is_openai_context_overflow_keyword(message: &str) -> bool {
    message.contains("maximum context length")
        || message.contains("context_length_exceeded")
        || message.contains("exceeds the maximum")
        || message.contains("too many tokens")
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
