//! Prompt hook executor — uses a lightweight LLM call as hook logic.
//!
//! Reuses the same pattern as `loopal-auto-mode/src/llm_call.rs`:
//! small max_tokens, temperature 0, no tools. The hook's `prompt` field
//! becomes the system prompt; the hook input JSON becomes the user message.

use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use loopal_error::HookError;
use loopal_provider_api::{ChatParams, Provider, StreamChunk};

use crate::executor::{HookExecutor, RawHookOutput};

/// Executes a hook by calling an LLM with a prompt.
pub struct PromptExecutor {
    pub system_prompt: String,
    pub model: String,
    pub provider: Arc<dyn Provider>,
    pub timeout: Duration,
    pub max_tokens: u32,
}

#[async_trait::async_trait]
impl HookExecutor for PromptExecutor {
    async fn execute(&self, input: serde_json::Value) -> Result<RawHookOutput, HookError> {
        let user_msg = serde_json::to_string_pretty(&input).unwrap_or_else(|_| input.to_string());

        let params = ChatParams {
            model: self.model.clone(),
            messages: vec![loopal_message::Message::user(&user_msg)],
            system_prompt: self.system_prompt.clone(),
            tools: vec![],
            max_tokens: self.max_tokens,
            temperature: Some(0.0),
            thinking: None,
            debug_dump_dir: None,
        };

        let text = tokio::time::timeout(self.timeout, self.stream_text(&params))
            .await
            .map_err(|_| {
                HookError::Timeout(format!(
                    "prompt hook timed out after {}ms",
                    self.timeout.as_millis()
                ))
            })??;

        // Try to extract exit_code from JSON response, default to 0.
        let exit_code = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("exit_code").and_then(|c| c.as_i64()))
            .unwrap_or(0) as i32;

        Ok(RawHookOutput {
            exit_code,
            stdout: text,
            stderr: String::new(),
        })
    }
}

impl PromptExecutor {
    async fn stream_text(&self, params: &ChatParams) -> Result<String, HookError> {
        let mut stream = self
            .provider
            .stream_chat(params)
            .await
            .map_err(|e| HookError::ExecutionFailed(e.to_string()))?;

        let mut text = String::new();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(StreamChunk::Text { text: t }) => text.push_str(&t),
                Ok(StreamChunk::Done { .. }) => break,
                Err(e) => return Err(HookError::ExecutionFailed(e.to_string())),
                _ => {} // ignore thinking, usage, etc.
            }
        }
        Ok(text)
    }
}
