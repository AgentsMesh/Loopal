use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OneShotChatError {
    Timeout,
    ProviderUnresolvable,
    StreamFailed,
    ChunkFailed,
    EmptyResponse,
}

impl std::fmt::Display for OneShotChatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Timeout => "LLM call exceeded timeout",
            Self::ProviderUnresolvable => "no provider for the requested model",
            Self::StreamFailed => "stream_chat failed before any chunks",
            Self::ChunkFailed => "streaming chunk failed mid-flight",
            Self::EmptyResponse => "LLM returned no text content",
        };
        f.write_str(s)
    }
}

impl std::error::Error for OneShotChatError {}

#[async_trait]
pub trait OneShotChatService: Send + Sync {
    async fn one_shot_chat(
        &self,
        model: &str,
        system_prompt: &str,
        user_prompt: &str,
        max_tokens: u32,
    ) -> Result<String, OneShotChatError>;
}

pub trait FetchRefinerPolicy: Send + Sync {
    /// Returns `Some(model)` when the fetch tool should run an LLM refiner
    /// over a body of `body_size` bytes; `None` to skip refinement (disabled
    /// in settings, or body below threshold).
    fn refiner_model(&self, body_size: usize) -> Option<String>;
}
