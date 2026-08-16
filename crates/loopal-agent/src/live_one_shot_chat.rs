use std::sync::Arc;

use async_trait::async_trait;
use loopal_provider_api::ThinkingConfigReader;
use loopal_tool_api::{OneShotChatEffort, OneShotChatError, OneShotChatService};

use crate::shared::AgentShared;

/// Production one-shot service bound to the runner's live thinking setting.
/// Legacy `AgentShared` callers retain startup-config behavior.
pub struct LiveOneShotChatService {
    agent: Arc<AgentShared>,
    thinking: ThinkingConfigReader,
}

impl LiveOneShotChatService {
    pub fn new(agent: Arc<AgentShared>, thinking: ThinkingConfigReader) -> Self {
        Self { agent, thinking }
    }

    async fn request(
        &self,
        model: &str,
        system_prompt: &str,
        user_prompt: &str,
        max_tokens: u32,
        effort: OneShotChatEffort,
    ) -> Result<String, OneShotChatError> {
        let configured = self.thinking.get();
        self.agent
            .one_shot_chat_inner(
                model,
                system_prompt,
                user_prompt,
                max_tokens,
                effort,
                &configured,
            )
            .await
    }
}

#[async_trait]
impl OneShotChatService for LiveOneShotChatService {
    async fn one_shot_chat(
        &self,
        model: &str,
        system_prompt: &str,
        user_prompt: &str,
        max_tokens: u32,
    ) -> Result<String, OneShotChatError> {
        self.request(
            model,
            system_prompt,
            user_prompt,
            max_tokens,
            OneShotChatEffort::Default,
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
        self.request(model, system_prompt, user_prompt, max_tokens, effort)
            .await
    }
}
