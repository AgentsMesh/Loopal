use async_trait::async_trait;
use loopagent_types::error::LoopAgentError;
use loopagent_types::provider::{ChatParams, ChatStream, Provider};

use crate::openai::OpenAiProvider;

/// OpenAI-compatible provider for services like Ollama, Together, vLLM, etc.
pub struct OpenAiCompatProvider {
    inner: OpenAiProvider,
    provider_name: String,
}

impl OpenAiCompatProvider {
    pub fn new(api_key: String, base_url: String, name: String) -> Self {
        Self {
            inner: OpenAiProvider::new(api_key).with_base_url(base_url),
            provider_name: name,
        }
    }
}

#[async_trait]
impl Provider for OpenAiCompatProvider {
    fn name(&self) -> &str {
        &self.provider_name
    }

    async fn stream_chat(&self, params: &ChatParams) -> Result<ChatStream, LoopAgentError> {
        self.inner.stream_chat(params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_returns_configured_name() {
        let provider = OpenAiCompatProvider::new(
            "key123".to_string(),
            "http://localhost:11434".to_string(),
            "ollama".to_string(),
        );
        assert_eq!(provider.name(), "ollama");
    }

    #[test]
    fn test_name_custom_provider() {
        let provider = OpenAiCompatProvider::new(
            "sk-test".to_string(),
            "https://api.together.xyz".to_string(),
            "together".to_string(),
        );
        assert_eq!(provider.name(), "together");
    }

    #[test]
    fn test_construction_sets_inner_provider() {
        let provider = OpenAiCompatProvider::new(
            "api-key".to_string(),
            "http://localhost:8080".to_string(),
            "vllm".to_string(),
        );
        // Verify the provider was constructed successfully and the name is correct
        assert_eq!(provider.provider_name, "vllm");
        // Inner provider should be an OpenAiProvider (we can verify by checking
        // that the struct was created without panic)
        assert_eq!(provider.inner.name(), "openai");
    }
}
