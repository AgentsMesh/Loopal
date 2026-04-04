mod message_builder;
mod stream;

use async_trait::async_trait;
use loopal_error::{LoopalError, ProviderError};
use loopal_provider_api::{ChatParams, ChatStream, Provider};
use serde_json::json;
use std::collections::VecDeque;
use std::time::Duration;

use crate::resilient_client::ResilientClient;
use crate::sse::SseStream;
use stream::ToolCallAccumulator;

/// OpenAI-compatible provider using Chat Completions API (`/v1/chat/completions`).
/// For services like DeepSeek, Ollama, Together, vLLM, etc.
pub struct OpenAiCompatProvider {
    client: ResilientClient,
    api_key: String,
    base_url: String,
    provider_name: String,
}

impl OpenAiCompatProvider {
    pub fn new(api_key: String, base_url: String, name: String) -> Self {
        Self {
            client: ResilientClient::new(Duration::from_secs(300), Duration::from_secs(10)),
            api_key,
            base_url,
            provider_name: name,
        }
    }
}

#[async_trait]
impl Provider for OpenAiCompatProvider {
    fn name(&self) -> &str {
        &self.provider_name
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
        body["stream_options"] = json!({"include_usage": true});

        tracing::info!(
            model = %params.model,
            url = %format!("{}/v1/chat/completions", self.base_url),
            messages = params.messages.len(),
            tools = params.tools.len(),
            "API request"
        );

        let (client, client_gen) = self.client.get();
        let response = client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
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
                return Err(ProviderError::RateLimited { retry_after_ms }.into());
            }
            let text = response.text().await.unwrap_or_default();
            tracing::error!(status = status.as_u16(), body = %text, "API error");
            return Err(ProviderError::Api {
                status: status.as_u16(),
                message: text,
            }
            .into());
        }

        let sse = SseStream::from_response(response);
        let stream = stream::CompatStream {
            inner: Box::pin(sse),
            state: ToolCallAccumulator::default(),
            buffer: VecDeque::new(),
        };
        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_returns_configured_name() {
        let provider = OpenAiCompatProvider::new(
            "key123".to_string(),
            "http://localhost:11434".to_string(),
            "ollama".to_string(),
        );
        assert_eq!(provider.name(), "ollama");
    }
}
