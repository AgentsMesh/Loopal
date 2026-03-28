/// MCP sampling adapter: bridges `SamplingCallback` to Loopal's `Provider` trait.
use std::sync::Arc;

use loopal_mcp::SamplingCallback;
use loopal_provider_api::{ChatParams, Provider, StreamChunk};
use tokio_stream::StreamExt;

/// Implements MCP sampling by calling a Loopal LLM provider.
pub struct McpSamplingAdapter {
    provider: Arc<dyn Provider>,
    model: String,
}

impl McpSamplingAdapter {
    pub fn new(provider: Arc<dyn Provider>, model: String) -> Self {
        Self { provider, model }
    }
}

#[async_trait::async_trait]
impl SamplingCallback for McpSamplingAdapter {
    async fn create_message(
        &self,
        system_prompt: Option<&str>,
        messages: Vec<(String, String)>,
        max_tokens: Option<u32>,
    ) -> Result<(String, String), String> {
        let llm_messages: Vec<loopal_message::Message> = messages
            .into_iter()
            .map(|(role, text)| match role.as_str() {
                "user" => loopal_message::Message::user(&text),
                _ => loopal_message::Message::assistant(&text),
            })
            .collect();

        let mut params = ChatParams::new(
            self.model.clone(),
            llm_messages,
            system_prompt
                .unwrap_or("You are a helpful assistant.")
                .to_string(),
        );
        if let Some(max) = max_tokens {
            params.max_tokens = max;
        }

        let mut stream = self
            .provider
            .stream_chat(&params)
            .await
            .map_err(|e| e.to_string())?;

        let mut text = String::new();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(StreamChunk::Text { text: delta }) => text.push_str(&delta),
                Ok(_) => {} // Skip thinking, usage, etc.
                Err(e) => return Err(e.to_string()),
            }
        }

        Ok((self.model.clone(), text))
    }
}
