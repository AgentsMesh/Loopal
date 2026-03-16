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

pub struct GoogleProvider {
    client: Client,
    api_key: String,
    base_url: String,
}

impl GoogleProvider {
    pub fn new(api_key: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(300))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("failed to build HTTP client");
        Self {
            client,
            api_key,
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    pub fn build_contents(&self, params: &ChatParams) -> Vec<Value> {
        params
            .messages
            .iter()
            .filter(|m| m.role != MessageRole::System)
            .map(|msg| {
                let role = match msg.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "model",
                    MessageRole::System => unreachable!(),
                };

                let parts: Vec<Value> = msg
                    .content
                    .iter()
                    .map(|block| match block {
                        ContentBlock::Text { text } => json!({"text": text}),
                        ContentBlock::ToolUse { name, input, .. } => json!({
                            "functionCall": {
                                "name": name,
                                "args": input
                            }
                        }),
                        ContentBlock::ToolResult {
                            tool_use_id: _,
                            content,
                            ..
                        } => json!({
                            "functionResponse": {
                                "name": "",
                                "response": {"result": content}
                            }
                        }),
                        ContentBlock::Image { source } => json!({
                            "inlineData": {
                                "mimeType": source.media_type,
                                "data": source.data
                            }
                        }),
                    })
                    .collect();

                json!({"role": role, "parts": parts})
            })
            .collect()
    }

    pub fn build_tools(&self, params: &ChatParams) -> Vec<Value> {
        if params.tools.is_empty() {
            return vec![];
        }

        let declarations: Vec<Value> = params
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.input_schema
                })
            })
            .collect();

        vec![json!({"functionDeclarations": declarations})]
    }
}

#[async_trait]
impl Provider for GoogleProvider {
    fn name(&self) -> &str {
        "google"
    }

    async fn stream_chat(&self, params: &ChatParams) -> Result<ChatStream, LoopAgentError> {
        // Normalize messages: filter system messages and merge consecutive same-role
        let normalized = loopagent_types::message_normalize::normalize_messages(&params.messages);
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

        let response = self
            .client
            .post(&url)
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
        let stream = stream::GoogleStream {
            inner: Box::pin(sse),
            buffer: VecDeque::new(),
        };
        Ok(Box::pin(stream))
    }
}
