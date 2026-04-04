//! Tests for PromptExecutor using MockProvider.

use std::sync::Arc;
use std::time::Duration;

use loopal_hooks::executor::HookExecutor;
use loopal_hooks::executor_prompt::PromptExecutor;
use loopal_provider_api::StreamChunk;
use loopal_test_support::mock_provider::MockProvider;

fn make_prompt_executor(response_text: &str) -> PromptExecutor {
    let chunks = vec![
        Ok(StreamChunk::Text {
            text: response_text.into(),
        }),
        Ok(StreamChunk::Done {
            stop_reason: loopal_provider_api::StopReason::EndTurn,
        }),
    ];
    let provider = Arc::new(MockProvider::new(chunks));
    PromptExecutor {
        system_prompt: "You are a hook.".into(),
        model: "test-model".into(),
        provider,
        timeout: Duration::from_secs(5),
        max_tokens: 256,
    }
}

#[tokio::test]
async fn test_prompt_executor_success() {
    let exec = make_prompt_executor("all good");
    let result = exec.execute(serde_json::json!({"tool": "Write"})).await;
    let output = result.unwrap();
    assert_eq!(output.exit_code, 0);
    assert!(output.stdout.contains("all good"));
}

#[tokio::test]
async fn test_prompt_executor_json_exit_code() {
    let exec = make_prompt_executor(r#"{"exit_code": 2, "reason": "blocked"}"#);
    let result = exec.execute(serde_json::json!({})).await;
    let output = result.unwrap();
    assert_eq!(output.exit_code, 2);
}

#[tokio::test]
async fn test_prompt_executor_no_exit_code_defaults_to_0() {
    let exec = make_prompt_executor(r#"{"feedback": "looks fine"}"#);
    let result = exec.execute(serde_json::json!({})).await;
    let output = result.unwrap();
    assert_eq!(output.exit_code, 0);
}
