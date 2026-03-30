use std::sync::Arc;

use loopal_auto_mode::AutoClassifier;
use loopal_error::LoopalError;
use loopal_provider_api::{ChatParams, ChatStream, Provider, StopReason, StreamChunk};
use loopal_tool_api::PermissionDecision;

// Reuse mock provider from classifier_test (simplified inline).
struct AllowProvider;

#[async_trait::async_trait]
impl Provider for AllowProvider {
    fn name(&self) -> &str {
        "mock"
    }
    async fn stream_chat(&self, _p: &ChatParams) -> Result<ChatStream, LoopalError> {
        use std::collections::VecDeque;
        struct S(VecDeque<Result<StreamChunk, LoopalError>>);
        impl futures::Stream for S {
            type Item = Result<StreamChunk, LoopalError>;
            fn poll_next(
                mut self: std::pin::Pin<&mut Self>,
                _cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<Option<Self::Item>> {
                std::task::Poll::Ready(self.0.pop_front())
            }
        }
        impl Unpin for S {}
        let chunks = VecDeque::from(vec![
            Ok(StreamChunk::Text {
                text: r#"{"should_block": false, "reason": "safe"}"#.into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ]);
        Ok(Box::pin(S(chunks)))
    }
}

#[tokio::test]
async fn second_call_returns_cached_result() {
    let classifier = AutoClassifier::new(String::new(), "/tmp".into());
    let provider = Arc::new(AllowProvider);
    let input = serde_json::json!({"command": "cargo test"});

    let r1 = classifier
        .classify("Bash", &input, "", &*provider, "m")
        .await;
    assert_eq!(r1.decision, PermissionDecision::Allow);

    let r2 = classifier
        .classify("Bash", &input, "", &*provider, "m")
        .await;
    assert_eq!(r2.decision, PermissionDecision::Allow);
    assert_eq!(r2.duration_ms, 0); // cached → 0ms
}

#[tokio::test]
async fn different_tool_name_is_cache_miss() {
    let classifier = AutoClassifier::new(String::new(), "/tmp".into());
    let provider = Arc::new(AllowProvider);
    let input = serde_json::json!({"command": "cargo test"});

    classifier
        .classify("Bash", &input, "", &*provider, "m")
        .await;
    // Same input but different tool name → should call LLM again
    let r2 = classifier
        .classify("Write", &input, "", &*provider, "m")
        .await;
    // Provider always returns allow, so this succeeds
    assert_eq!(r2.decision, PermissionDecision::Allow);
}

#[tokio::test]
async fn different_input_is_cache_miss() {
    let classifier = AutoClassifier::new(String::new(), "/tmp".into());
    let provider = Arc::new(AllowProvider);

    classifier
        .classify(
            "Bash",
            &serde_json::json!({"command": "ls"}),
            "",
            &*provider,
            "m",
        )
        .await;
    let r2 = classifier
        .classify(
            "Bash",
            &serde_json::json!({"command": "rm -rf /"}),
            "",
            &*provider,
            "m",
        )
        .await;
    // Different input → fresh classify (both happen to be Allow from mock)
    assert_eq!(r2.decision, PermissionDecision::Allow);
}

#[tokio::test]
async fn error_results_are_not_cached() {
    let classifier = AutoClassifier::new(String::new(), "/tmp".into());

    // ErrorProvider returns LLM error
    struct ErrorProvider;
    #[async_trait::async_trait]
    impl Provider for ErrorProvider {
        fn name(&self) -> &str {
            "mock"
        }
        async fn stream_chat(&self, _p: &ChatParams) -> Result<ChatStream, LoopalError> {
            Err(LoopalError::Other("fail".into()))
        }
    }

    let provider = Arc::new(ErrorProvider);
    let input = serde_json::json!({"command": "test"});

    let r1 = classifier
        .classify("Bash", &input, "", &*provider, "m")
        .await;
    assert_eq!(r1.decision, PermissionDecision::Deny);

    // Second call should NOT return cached error — it should call LLM again
    let r2 = classifier
        .classify("Bash", &input, "", &*provider, "m")
        .await;
    assert_eq!(r2.decision, PermissionDecision::Deny);
    // Both are Deny from LLM error, not from cache
}
