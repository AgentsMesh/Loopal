mod message_builder;
mod stream;
mod thinking;

use async_trait::async_trait;
use loopal_error::{LoopalError, ProviderError};
use loopal_provider_api::{
    ChatParams, ChatStream, ErrorClass, Provider, ThinkingCapability, default_classify_error,
};
use serde_json::json;
use std::collections::VecDeque;
use std::time::Duration;
use tracing::Instrument;

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
        // reason: build_messages still consumes Vec<Message>; project turns →
        // messages locally. OpenAI Chat Completions API tolerates assistant-tail
        // so no continuation user-tail needed.
        let projected = loopal_provider_api::project_turns_to_messages(&params.turns);
        let normalized = loopal_provider_api::normalize_messages(&projected);
        let messages = self.build_messages_from_messages(&normalized, params);
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
        if let Some(ref config) = params.thinking
            && let Some(effort) = thinking::reasoning_effort(config)?
        {
            body["reasoning_effort"] = json!(effort);
        }
        body["stream_options"] = json!({"include_usage": true});

        tracing::info!(
            model = %params.model,
            provider = "openai-compatible",
            messages = normalized.len(),
            tools = params.tools.len(),
            "API request"
        );

        let endpoint = crate::endpoint::join_v1(&self.base_url, "/v1/chat/completions");
        let http_span = crate::http_telemetry::request_span("openai_compat", &endpoint);
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
                return Err(crate::safe_diagnostics::network_error("openai-compatible", &e).into());
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
                "openai-compatible",
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
        let stream = stream::CompatStream {
            inner: Box::pin(sse),
            state: ToolCallAccumulator::default(),
            buffer: VecDeque::new(),
            emit_reasoning: crate::get_thinking_capability(&params.model)
                != ThinkingCapability::None,
        };
        Ok(Box::pin(stream))
    }

    fn classify_error(&self, err: &LoopalError) -> ErrorClass {
        if let LoopalError::Provider(ProviderError::Api {
            status: 400,
            message,
            ..
        }) = err
            && is_openai_compat_context_overflow_keyword(message)
        {
            return ErrorClass::ContextOverflow;
        }
        default_classify_error(err)
    }
}

fn is_openai_compat_context_overflow_keyword(message: &str) -> bool {
    message.contains("maximum context length")
        || message.contains("context_length_exceeded")
        || message.contains("exceeds the maximum")
        || message.contains("too many tokens")
        || message.contains("prompt is too long")
}

#[cfg(test)]
mod tests {
    use super::*;
    use loopal_provider_api::{ContentBlock, ImageSource, Message};

    #[test]
    fn test_name_returns_configured_name() {
        let provider = OpenAiCompatProvider::new(
            "key123".to_string(),
            "http://localhost:11434".to_string(),
            "ollama".to_string(),
        );
        assert_eq!(provider.name(), "ollama");
    }

    #[test]
    fn caption_and_image_share_one_user_message() {
        let provider = OpenAiCompatProvider::new("key".into(), "http://mock".into(), "mock".into());
        let mut message = Message::user("caption");
        message.content.push(ContentBlock::Image {
            source: ImageSource {
                source_type: "base64".into(),
                media_type: "image/png".into(),
                data: "AA==".into(),
            },
        });
        let params = ChatParams::new("mock".into(), vec![], String::new());
        let messages = provider.build_messages_from_messages(&[message], &params);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[0]["content"][0]["text"], "caption");
        assert_eq!(messages[0]["content"][1]["type"], "image_url");
    }
}
