use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use loopal_provider::{get_thinking_capability, resolve_thinking_config_with_recommendation};
use loopal_provider_api::{ChatParams, EffortLevel, StreamChunk, TaskType, ThinkingConfig};
use loopal_tool_api::{
    FetchRefinerPolicy, OneShotChatEffort, OneShotChatError, OneShotChatService,
};

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
        self.one_shot_chat_inner(
            model,
            system_prompt,
            user_prompt,
            max_tokens,
            OneShotChatEffort::Default,
            &self.kernel.settings().thinking,
        )
        .await
    }

    async fn one_shot_chat_with_effort(
        &self,
        model: &str,
        system_prompt: &str,
        user_prompt: &str,
        max_tokens: u32,
        effort: OneShotChatEffort,
    ) -> Result<String, OneShotChatError> {
        self.one_shot_chat_inner(
            model,
            system_prompt,
            user_prompt,
            max_tokens,
            effort,
            &self.kernel.settings().thinking,
        )
        .await
    }
}

impl AgentShared {
    pub(crate) async fn one_shot_chat_inner(
        &self,
        model: &str,
        system_prompt: &str,
        user_prompt: &str,
        max_tokens: u32,
        effort: OneShotChatEffort,
        configured: &ThinkingConfig,
    ) -> Result<String, OneShotChatError> {
        let provider = self
            .kernel
            .resolve_provider(model)
            .map_err(|_| OneShotChatError::ProviderUnresolvable)?;
        let params = ChatParams {
            model: model.to_string(),
            turns: vec![loopal_turn::Turn::single_user_prompt(user_prompt)],
            system_prompt: system_prompt.to_string(),
            tools: vec![],
            max_tokens,
            temperature: Some(0.0),
            thinking: one_shot_thinking(model, max_tokens, effort, configured),
            continuation_intent: None,
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

fn one_shot_thinking(
    model: &str,
    max_tokens: u32,
    effort: OneShotChatEffort,
    configured: &ThinkingConfig,
) -> Option<ThinkingConfig> {
    if !matches!(effort, OneShotChatEffort::Max) {
        return None;
    }
    let recommendation = ThinkingConfig::Effort {
        level: EffortLevel::Max,
    };
    resolve_thinking_config_with_recommendation(
        configured,
        Some(&recommendation),
        get_thinking_capability(model),
        max_tokens,
    )
    .unwrap_or_else(|error| {
        tracing::warn!(%error, %model, "workflow planner thinking hint was not supported");
        None
    })
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

#[cfg(test)]
#[path = "provider_resolver_service_tests.rs"]
mod service_tests;
#[cfg(test)]
#[path = "provider_resolver_impl_tests.rs"]
mod tests;
