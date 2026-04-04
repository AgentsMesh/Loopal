mod request;
pub(crate) mod server_tool;
mod stream;
mod thinking;

use async_trait::async_trait;
use loopal_error::{LoopalError, ProviderError};
use loopal_provider_api::{ChatParams, ChatStream, Provider};
use serde_json::json;
use std::collections::VecDeque;
use std::time::Duration;

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
        let normalized = loopal_message::normalize_messages(&params.messages);
        let normalized_params = ChatParams {
            messages: normalized,
            ..params.clone()
        };
        let contents = self.build_contents(&normalized_params);
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
            messages = params.messages.len(),
            tools = params.tools.len(),
            max_tokens = params.max_tokens,
            "API request"
        );

        let (client, client_gen) = self.client.get();
        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                self.client.report_network_error(client_gen);
                ProviderError::Http(format!("{e:#}"))
            })?;
        self.client.report_success(client_gen);

        let status = response.status();
        tracing::info!(status = status.as_u16(), "API response");
        if !status.is_success() {
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
        let stream = stream::GoogleStream {
            inner: Box::pin(sse),
            buffer: VecDeque::new(),
        };
        Ok(Box::pin(stream))
    }
}
