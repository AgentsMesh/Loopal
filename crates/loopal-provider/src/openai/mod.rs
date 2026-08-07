mod input_builder;
pub(crate) mod server_tool;
mod stream;
mod stream_error;
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
        // reason: build_input still consumes Vec<Message>; project turns →
        // messages locally so existing build path is unchanged. Responses API
        // tolerates assistant-tail so no continuation user-tail needed here.
        let messages = loopal_provider_api::project_turns_to_messages(&params.turns);
        let input = self.build_input_from_messages(&messages, params);
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
            body["reasoning"] = thinking::to_openai_reasoning(thinking_config)?;
        } else if let Some(temp) = params.temperature {
            body["temperature"] = json!(temp);
        }

        tracing::info!(
            model = %params.model,
            provider = "openai",
            messages = messages.len(),
            tools = tools.len(),
            max_tokens = params.max_tokens,
            "API request"
        );

        let endpoint = crate::endpoint::join_v1(&self.base_url, "/v1/responses");
        let http_span = crate::http_telemetry::request_span("openai", &endpoint);
        let (client, client_gen) = self.client.get();
        let response = client
            .post(endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .instrument(http_span.clone())
            .await;
        let response = match response {
            Ok(response) => response,
            Err(e) => {
                crate::http_telemetry::record_transport_error(&http_span);
                self.client.report_network_error(client_gen);
                return Err(crate::safe_diagnostics::network_error("openai", &e).into());
            }
        };
        self.client.report_success(client_gen);

        let status = response.status();
        crate::http_telemetry::record_response(&http_span, status);
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
            ..
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
        let retry_after_ms = crate::retry_after::from_headers(response.headers());
        if status.as_u16() == 429 {
            let error = crate::retry_after::provider_error(
                status.as_u16(),
                "rate limited by API".into(),
                retry_after_ms,
            );
            let retry_after_ms = error.retry_after_ms().unwrap_or_default();
            tracing::warn!(retry_after_ms, "rate limited by API");
            return error.into();
        }
        let text = crate::safe_diagnostics::response_error_message(
            "openai",
            response,
            &[&self.api_key, &self.base_url],
        )
        .await;
        tracing::error!(status = status.as_u16(), "API error");
        crate::retry_after::provider_error(status.as_u16(), text, retry_after_ms).into()
    }
}
