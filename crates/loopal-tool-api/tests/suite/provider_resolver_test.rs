use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use loopal_tool_api::{
    OneShotChatEffort, OneShotChatError, OneShotChatOptions, OneShotChatService,
};

struct LegacyOnly;

#[async_trait]
impl OneShotChatService for LegacyOnly {
    async fn one_shot_chat(
        &self,
        _model: &str,
        _system_prompt: &str,
        _user_prompt: &str,
        _max_tokens: u32,
    ) -> Result<String, OneShotChatError> {
        Ok("legacy".into())
    }
}

struct EffortAware {
    seen: Arc<Mutex<Option<OneShotChatEffort>>>,
}

#[async_trait]
impl OneShotChatService for EffortAware {
    async fn one_shot_chat(
        &self,
        _model: &str,
        _system_prompt: &str,
        _user_prompt: &str,
        _max_tokens: u32,
    ) -> Result<String, OneShotChatError> {
        Ok("legacy".into())
    }

    async fn one_shot_chat_with_effort(
        &self,
        _model: &str,
        _system_prompt: &str,
        _user_prompt: &str,
        _max_tokens: u32,
        effort: OneShotChatEffort,
    ) -> Result<String, OneShotChatError> {
        *self.seen.lock().unwrap() = Some(effort);
        Ok("effort".into())
    }
}

#[tokio::test]
async fn options_keep_legacy_implementations_source_compatible() {
    let service = LegacyOnly;
    let response = service
        .one_shot_chat_with_options(
            "model",
            "system",
            "user",
            128,
            OneShotChatOptions::new(OneShotChatEffort::Max),
        )
        .await
        .unwrap();
    assert_eq!(response, "legacy");
}

#[tokio::test]
async fn options_route_through_existing_effort_hook() {
    let seen = Arc::new(Mutex::new(None));
    let service = EffortAware { seen: seen.clone() };
    let response = service
        .one_shot_chat_with_options(
            "model",
            "system",
            "user",
            128,
            OneShotChatEffort::Max.into(),
        )
        .await
        .unwrap();
    assert_eq!(response, "effort");
    assert_eq!(*seen.lock().unwrap(), Some(OneShotChatEffort::Max));
}
