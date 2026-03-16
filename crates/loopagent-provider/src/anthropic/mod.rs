mod stream;

use async_trait::async_trait;
use loopagent_types::error::{LoopAgentError, ProviderError};
use loopagent_types::message::{ContentBlock, MessageRole};
use loopagent_types::provider::{ChatParams, ChatStream, Provider};
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::time::Duration;

use crate::sse::SseStream;
use stream::ToolUseAccumulator;

pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    base_url: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(300))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("failed to build HTTP client");
        Self {
            client,
            api_key,
            base_url: "https://api.anthropic.com".to_string(),
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    pub fn build_messages(&self, params: &ChatParams) -> Vec<Value> {
        params
            .messages
            .iter()
            .filter(|m| m.role != MessageRole::System)
            .map(|msg| {
                let role = match msg.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::System => unreachable!(),
                };

                let content: Vec<Value> = msg
                    .content
                    .iter()
                    .map(|block| match block {
                        ContentBlock::Text { text } => json!({
                            "type": "text",
                            "text": text
                        }),
                        ContentBlock::ToolUse { id, name, input } => json!({
                            "type": "tool_use",
                            "id": id,
                            "name": name,
                            "input": input
                        }),
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } => json!({
                            "type": "tool_result",
                            "tool_use_id": tool_use_id,
                            "content": content,
                            "is_error": is_error
                        }),
                        ContentBlock::Image { source } => json!({
                            "type": "image",
                            "source": {
                                "type": source.source_type,
                                "media_type": source.media_type,
                                "data": source.data
                            }
                        }),
                    })
                    .collect();

                json!({
                    "role": role,
                    "content": content
                })
            })
            .collect()
    }

    pub fn build_tools(&self, params: &ChatParams) -> Vec<Value> {
        params
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "input_schema": tool.input_schema
                })
            })
            .collect()
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn stream_chat(&self, params: &ChatParams) -> Result<ChatStream, LoopAgentError> {
        // Normalize messages: filter system messages and merge consecutive same-role
        let normalized = loopagent_types::message_normalize::normalize_messages(&params.messages);
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
            body["system"] = json!(params.system_prompt);
        }
        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }
        if let Some(temp) = params.temperature {
            body["temperature"] = json!(temp);
        }

        tracing::info!(
            model = %params.model,
            url = %format!("{}/v1/messages", self.base_url),
            messages = params.messages.len(),
            tools = params.tools.len(),
            max_tokens = params.max_tokens,
            "API request"
        );

        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
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
        let stream = stream::AnthropicStream {
            inner: Box::pin(sse),
            state: ToolUseAccumulator::default(),
            buffer: VecDeque::new(),
        };
        Ok(Box::pin(stream))
    }
}
