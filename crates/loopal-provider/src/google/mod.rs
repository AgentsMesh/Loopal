mod request;
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

pub struct GoogleProvider {
    client: ResilientClient,
    api_key: String,
    base_url: String,
}

impl GoogleProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: ResilientClient::new(Duration::from_secs(300), Duration::from_secs(10)),
            api_key,
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }
}

#[async_trait]
impl Provider for GoogleProvider {
    fn name(&self) -> &str {
        "google"
    }

    async fn stream_chat(&self, params: &ChatParams) -> Result<ChatStream, LoopalError> {
        // reason: build_contents still consumes Vec<Message>; project turns →
        // messages locally then normalize (Google requires alternating roles
        // like Anthropic). One projection + one normalize, no ChatParams clone.
        let projected = loopal_provider_api::project_turns_to_messages(&params.turns);
        let messages = loopal_provider_api::normalize_messages(&projected);
        let contents = self.build_contents_from_messages(&messages, params);
        let tools = self.build_tools(params);

        let mut body = json!({
            "contents": contents,
            "generationConfig": {
                "maxOutputTokens": params.max_tokens,
            },
        });

        if !params.system_prompt.is_empty() {
            body["systemInstruction"] = json!({
                "parts": [{"text": params.system_prompt}]
            });
        }
        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }
        if let Some(temp) = params.temperature {
            body["generationConfig"]["temperature"] = json!(temp);
        }
        if let Some(ref thinking_config) = params.thinking {
            body["generationConfig"]["thinkingConfig"] =
                thinking::to_google_thinking(thinking_config);
        }

        let url = format!(
            "{}/models/{}:streamGenerateContent?alt=sse&key={}",
            self.base_url, params.model, self.api_key
        );

        tracing::info!(
            model = %params.model,
            messages = messages.len(),
            tools = params.tools.len(),
            max_tokens = params.max_tokens,
            "API request"
        );

        let http_span = crate::http_telemetry::request_span("google", &url);
        let (client, client_gen) = self.client.get();
        let response = client
            .post(&url)
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
                return Err(crate::safe_diagnostics::network_error("google", &e).into());
            }
        };
        self.client.report_success(client_gen);

        let status = response.status();
        crate::http_telemetry::record_response(&http_span, status);
        tracing::info!(status = status.as_u16(), "API response");
        if !status.is_success() {
            let retry_after_ms = crate::retry_after::from_headers(response.headers());
            if status.as_u16() == 429 {
                return Err(crate::retry_after::provider_error(
                    status.as_u16(),
                    "rate limited by API".into(),
                    retry_after_ms,
                )
                .into());
            }
            let text = crate::safe_diagnostics::response_error_message(
                "google",
                response,
                &[&self.api_key, &self.base_url],
            )
            .await;
            tracing::error!(status = status.as_u16(), "API error");
            return Err(
                crate::retry_after::provider_error(status.as_u16(), text, retry_after_ms).into(),
            );
        }

        let sse = SseStream::from_response(response);
        let stream = stream::GoogleStream {
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
            && is_google_context_overflow_keyword(message)
        {
            return ErrorClass::ContextOverflow;
        }
        default_classify_error(err)
    }
}

fn is_google_context_overflow_keyword(message: &str) -> bool {
    // Gemini API surfaces context-window errors with these substrings.
    message.contains("token count")
        || message.contains("exceeds the maximum")
        || message.contains("input is too long")
}
