/// MCP sampling adapter: bridges `SamplingCallback` to Loopal's `Provider` trait.
use std::sync::Arc;

use loopal_mcp::SamplingCallback;
use loopal_provider_api::{ChatParams, Provider, StreamChunk};
use loopal_turn::Turn;
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
        // reason: MCP sampling delivers a flat role+text history. Loopal's
        // provider takes Turns — concatenate the history into a single user
        // prompt as the trigger (assistant-side text becomes part of the
        // prompt with a marker prefix). MCP sampling rarely needs deep
        // history; if it does, this can be promoted to per-role Turns later.
        let mut combined = String::new();
        for (role, text) in messages {
            if !combined.is_empty() {
                combined.push_str("\n\n");
            }
            combined.push_str(&format!("[{role}] {text}"));
        }
        let mut params = ChatParams::new(
            self.model.clone(),
            vec![Turn::single_user_prompt(combined)],
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
