use std::collections::VecDeque;
use std::sync::Arc;

use loopal_auto_mode::AutoClassifier;
use loopal_error::LoopalError;
use loopal_provider_api::{ChatParams, ChatStream, Provider, StopReason, StreamChunk};
use loopal_tool_api::PermissionDecision;

// ── Mock provider ──────────────────────────────────────────────────

struct MockClassifierProvider {
    response: std::sync::Mutex<Option<Result<String, LoopalError>>>,
}

impl MockClassifierProvider {
    fn ok(json: &str) -> Arc<Self> {
        Arc::new(Self {
            response: std::sync::Mutex::new(Some(Ok(json.to_string()))),
        })
    }

    fn err() -> Arc<Self> {
        Arc::new(Self {
            response: std::sync::Mutex::new(Some(Err(LoopalError::Other("mock error".into())))),
        })
    }
}

struct MockStream(VecDeque<Result<StreamChunk, LoopalError>>);
impl futures::Stream for MockStream {
    type Item = Result<StreamChunk, LoopalError>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::task::Poll::Ready(self.0.pop_front())
    }
}
impl Unpin for MockStream {}

#[async_trait::async_trait]
impl Provider for MockClassifierProvider {
    fn name(&self) -> &str {
        "mock"
    }
    async fn stream_chat(&self, _p: &ChatParams) -> Result<ChatStream, LoopalError> {
        let resp = self.response.lock().unwrap().take().unwrap();
        match resp {
            Ok(text) => {
                let chunks = VecDeque::from(vec![
                    Ok(StreamChunk::Text { text }),
                    Ok(StreamChunk::Done {
                        stop_reason: StopReason::EndTurn,
                    }),
                ]);
                Ok(Box::pin(MockStream(chunks)))
            }
            Err(e) => Err(e),
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[tokio::test]
async fn allow_response_returns_allow() {
    let provider =
        MockClassifierProvider::ok(r#"{"should_block": false, "reason": "Normal test execution"}"#);
    let classifier = AutoClassifier::new(String::new(), "/tmp/test".into());
    let result = classifier
        .classify(
            "Bash",
            &serde_json::json!({"command": "cargo test"}),
            "",
            &*provider,
            "test",
        )
        .await;
    assert_eq!(result.decision, PermissionDecision::Allow);
    assert_eq!(result.reason, "Normal test execution");
}

#[tokio::test]
async fn block_response_returns_deny() {
    let provider = MockClassifierProvider::ok(
        r#"{"should_block": true, "reason": "Dangerous delete command"}"#,
    );
    let classifier = AutoClassifier::new(String::new(), "/tmp/test".into());
    let result = classifier
        .classify(
            "Bash",
            &serde_json::json!({"command": "rm -rf /"}),
            "",
            &*provider,
            "test",
        )
        .await;
    assert_eq!(result.decision, PermissionDecision::Deny);
    assert_eq!(result.reason, "Dangerous delete command");
}

#[tokio::test]
async fn markdown_fenced_json_parsed() {
    let provider =
        MockClassifierProvider::ok("```json\n{\"should_block\": false, \"reason\": \"ok\"}\n```");
    let classifier = AutoClassifier::new(String::new(), "/tmp/test".into());
    let result = classifier
        .classify("Bash", &serde_json::json!({}), "", &*provider, "test")
        .await;
    assert_eq!(result.decision, PermissionDecision::Allow);
}

#[tokio::test]
async fn malformed_json_returns_deny() {
    let provider = MockClassifierProvider::ok("not json at all");
    let classifier = AutoClassifier::new(String::new(), "/tmp/test".into());
    let result = classifier
        .classify("Bash", &serde_json::json!({}), "", &*provider, "test")
        .await;
    assert_eq!(result.decision, PermissionDecision::Deny);
    assert!(result.reason.contains("parse failure"));
}

#[tokio::test]
async fn empty_response_returns_deny() {
    let provider = MockClassifierProvider::ok("");
    let classifier = AutoClassifier::new(String::new(), "/tmp/test".into());
    let result = classifier
        .classify("Bash", &serde_json::json!({}), "", &*provider, "test")
        .await;
    assert_eq!(result.decision, PermissionDecision::Deny);
}

#[tokio::test]
async fn provider_error_returns_deny() {
    let provider = MockClassifierProvider::err();
    let classifier = AutoClassifier::new(String::new(), "/tmp/test".into());
    let result = classifier
        .classify("Bash", &serde_json::json!({}), "", &*provider, "test")
        .await;
    assert_eq!(result.decision, PermissionDecision::Deny);
    assert!(result.reason.contains("error"));
}

#[tokio::test]
async fn circuit_breaker_degrades_after_repeated_errors() {
    let classifier = AutoClassifier::new(String::new(), "/tmp/test".into());
    // 3 errors → degraded
    for _ in 0..3 {
        let provider = MockClassifierProvider::err();
        classifier
            .classify("Bash", &serde_json::json!({}), "", &*provider, "test")
            .await;
    }
    assert!(classifier.is_degraded());
}

#[tokio::test]
async fn human_approval_resets_degradation() {
    let classifier = AutoClassifier::new(String::new(), "/tmp/test".into());
    for _ in 0..3 {
        let provider = MockClassifierProvider::err();
        classifier
            .classify("Bash", &serde_json::json!({}), "", &*provider, "test")
            .await;
    }
    assert!(classifier.is_degraded());
    classifier.on_human_approval("Bash");
    assert!(!classifier.is_degraded());
}
