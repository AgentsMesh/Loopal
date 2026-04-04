//! Tests for DefaultExecutorFactory dispatch logic.

use std::sync::Arc;

use loopal_config::{HookConfig, HookEvent, HookType};
use loopal_hooks::executor::ExecutorFactory;
use loopal_kernel::hook_factory::DefaultExecutorFactory;

fn make_config(hook_type: HookType) -> HookConfig {
    HookConfig {
        event: HookEvent::PostToolUse,
        hook_type,
        command: "echo test".into(),
        url: Some("http://localhost:9999".into()),
        headers: Default::default(),
        prompt: Some("test prompt".into()),
        model: None,
        tool_filter: None,
        condition: None,
        timeout_ms: 5000,
        id: None,
    }
}

#[tokio::test]
async fn test_factory_creates_command_executor() {
    let factory = DefaultExecutorFactory::new(None);
    let executor = factory.create(&make_config(HookType::Command)).unwrap();
    let result = executor.execute(serde_json::json!({})).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().exit_code, 0);
}

#[tokio::test]
async fn test_factory_creates_http_executor() {
    let factory = DefaultExecutorFactory::new(None);
    let config = make_config(HookType::Http);
    let executor = factory.create(&config).unwrap();
    let result = executor.execute(serde_json::json!({})).await;
    // Connection to localhost:9999 should fail (either refused or timeout)
    assert!(result.is_err() || result.as_ref().is_ok_and(|r| r.exit_code != 0));
}

#[tokio::test]
async fn test_factory_prompt_without_provider_returns_none() {
    let factory = DefaultExecutorFactory::new(None);
    assert!(factory.create(&make_config(HookType::Prompt)).is_none());
}

#[tokio::test]
async fn test_factory_prompt_with_provider_creates_executor() {
    use loopal_provider_api::StreamChunk;
    use loopal_test_support::mock_provider::MockProvider;

    let chunks = vec![
        Ok(StreamChunk::Text { text: "ok".into() }),
        Ok(StreamChunk::Done {
            stop_reason: loopal_provider_api::StopReason::EndTurn,
        }),
    ];
    let provider: Arc<dyn loopal_provider_api::Provider> = Arc::new(MockProvider::new(chunks));
    let factory = DefaultExecutorFactory::new(Some(provider));
    let executor = factory.create(&make_config(HookType::Prompt)).unwrap();
    let result = executor.execute(serde_json::json!({})).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().exit_code, 0);
}

#[tokio::test]
async fn test_factory_http_missing_url_returns_none() {
    let factory = DefaultExecutorFactory::new(None);
    let mut config = make_config(HookType::Http);
    config.url = None;
    assert!(factory.create(&config).is_none());
}
