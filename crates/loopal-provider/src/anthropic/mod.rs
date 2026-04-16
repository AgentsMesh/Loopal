mod accumulator;
mod request;
pub(crate) mod server_tool;
mod stream;
mod stream_parser;
mod thinking;

use async_trait::async_trait;
use loopal_error::{LoopalError, ProviderError};
use loopal_provider_api::{ChatParams, ChatStream, Provider};
use serde_json::json;
use std::collections::VecDeque;
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::Instrument;

use crate::resilient_client::ResilientClient;
use crate::sse::SseStream;
use stream::{ServerToolAccumulator, ThinkingAccumulator, ToolUseAccumulator};

/// Maximum concurrent API requests to avoid overwhelming the upstream proxy/API.
const MAX_CONCURRENT_REQUESTS: usize = 3;

pub struct AnthropicProvider {
    client: ResilientClient,
    api_key: String,
    base_url: String,
    /// Limits concurrent in-flight requests across all agents sharing this provider.
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
        // Acquire permit before sending — blocks if too many concurrent requests.
        let _permit = self
            .request_semaphore
            .acquire()
            .await
            .map_err(|_| ProviderError::Http("request semaphore closed".into()))?;

        let stream = self.do_stream_chat(params).await?;

        // Permit is dropped here, but the SSE stream continues reading.
        // This is intentional: we only gate the initial HTTP request, not the full stream lifetime.
        Ok(stream)
    }
}

impl AnthropicProvider {
    /// Inner implementation of stream_chat, called after acquiring the semaphore permit.
    async fn do_stream_chat(&self, params: &ChatParams) -> Result<ChatStream, LoopalError> {
        let normalized = loopal_message::normalize_messages(&params.messages);
        let normalized_params = ChatParams {
            messages: normalized,
            ..params.clone()
        };
        let messages = self.build_messages(&normalized_params);
        let tools = self.build_tools(params);

        let mut body = json!({
            "model": params.model,
            "max_tokens": params.max_tokens,
            "stream": true,
            "messages": messages,
        });

        if !params.system_prompt.is_empty() {
            body["system"] = json!([{
                "type": "text",
                "text": params.system_prompt,
                "cache_control": {"type": "ephemeral"}
            }]);
        }
        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }
        if let Some(temp) = params.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(ref thinking_config) = params.thinking {
            body["thinking"] = thinking::to_anthropic_thinking(thinking_config, params.max_tokens);
            if let Some(output_config) = thinking::to_anthropic_output_config(thinking_config) {
                body["output_config"] = output_config;
            }
        }

        tracing::info!(
            model = %params.model, url = %format!("{}/v1/messages", self.base_url),
            messages = params.messages.len(), tools = params.tools.len(),
            max_tokens = params.max_tokens,
            body_bytes = body.to_string().len(),
            "API request"
        );

        let http_span = tracing::info_span!("http_request", gen_ai.system = "anthropic");
        let (client, client_gen) = self.client.get();
        let response = client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
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
            // Dump request body on error for diagnosis
            if let Some(ref dump_dir) = params.debug_dump_dir {
                let _ = std::fs::create_dir_all(dump_dir);
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0);
                let path = dump_dir.join(format!("api_error_{status}_{ts}.json"));
                let _ = std::fs::write(&path, body.to_string());
                tracing::warn!(path = %path.display(), "dumped failed request body");
            }
            return Err(self.handle_error_response(response, status).await);
        }

        let sse = SseStream::from_response(response);
        let stream = stream::AnthropicStream {
            inner: Box::pin(sse),
            tool_state: ToolUseAccumulator::default(),
            thinking_state: ThinkingAccumulator::default(),
            server_tool_state: ServerToolAccumulator::default(),
            buffer: VecDeque::new(),
        };
        Ok(Box::pin(stream))
    }

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

        // Detect context overflow: 400 + known prompt-too-long patterns.
        // Intentionally excludes "invalid_request_error" — that type covers
        // many 400 errors (prefill rejection, malformed blocks, etc.) and
        // must not be conflated with context overflow.
        if status.as_u16() == 400
            && (text.contains("prompt is too long")
                || text.contains("maximum context length")
                || text.contains("exceed context limit"))
        {
            return ProviderError::ContextOverflow { message: text }.into();
        }

        ProviderError::Api {
            status: status.as_u16(),
            message: text,
        }
        .into()
    }
}
