use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use loopal_message::Message;
use loopal_provider_api::{ChatParams, StreamChunk, TaskType};
use loopal_tool_api::{FetchRefinerPolicy, OneShotChatError, OneShotChatService};

use crate::shared::AgentShared;

const ONE_SHOT_TIMEOUT: Duration = Duration::from_secs(30);

#[async_trait]
impl OneShotChatService for AgentShared {
    async fn one_shot_chat(
        &self,
        model: &str,
        system_prompt: &str,
        user_prompt: &str,
        max_tokens: u32,
    ) -> Result<String, OneShotChatError> {
        let provider = self
            .kernel
            .resolve_provider(model)
            .map_err(|_| OneShotChatError::ProviderUnresolvable)?;
        let params = ChatParams {
            model: model.to_string(),
            messages: vec![Message::user(user_prompt)],
            system_prompt: system_prompt.to_string(),
            tools: vec![],
            max_tokens,
            temperature: Some(0.0),
            thinking: None,
            debug_dump_dir: None,
        };
        let result = tokio::time::timeout(ONE_SHOT_TIMEOUT, async {
            let mut stream = provider
                .stream_chat(&params)
                .await
                .map_err(|_| OneShotChatError::StreamFailed)?;
            let mut out = String::new();
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(StreamChunk::Text { text }) => out.push_str(&text),
                    Ok(StreamChunk::Done { .. }) => break,
                    Err(_) => return Err(OneShotChatError::ChunkFailed),
                    _ => {}
                }
            }
            if out.is_empty() {
                Err(OneShotChatError::EmptyResponse)
            } else {
                Ok(out)
            }
        })
        .await;
        match result {
            Ok(inner) => inner,
            Err(_) => Err(OneShotChatError::Timeout),
        }
    }
}

impl FetchRefinerPolicy for AgentShared {
    fn refiner_model(&self, body_size: usize) -> Option<String> {
        let s = self.kernel.settings();
        if !s.fetch_refiner.enabled || body_size <= s.fetch_refiner.threshold_bytes {
            return None;
        }
        s.model_routing.get(&TaskType::Refine).cloned()
    }
}
