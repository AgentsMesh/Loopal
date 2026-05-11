use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use loopal_auto_mode::AutoClassifier;
use loopal_error::LoopalError;
use loopal_provider_api::{
    ChatParams, ChatStream, Provider, ProviderResolver, StopReason, StreamChunk, TaskType,
};
use loopal_runtime::frontend::permission_handler::{PermissionHandler, PermissionOutcome};
use loopal_runtime::frontend::{AutoPermissionHandler, DecisionContext, DegradedAction};
use loopal_tool_api::PermissionDecision;

struct FailingResolver;

impl ProviderResolver for FailingResolver {
    fn resolve_for(&self, _task: TaskType) -> Result<(String, Arc<dyn Provider>), LoopalError> {
        Err(LoopalError::Other("test resolver failure".into()))
    }
}

struct MockResolver {
    provider: Arc<dyn Provider>,
    model: String,
}

impl ProviderResolver for MockResolver {
    fn resolve_for(&self, _task: TaskType) -> Result<(String, Arc<dyn Provider>), LoopalError> {
        Ok((self.model.clone(), self.provider.clone()))
    }
}

struct MockProvider {
    response: std::sync::Mutex<Option<String>>,
}

impl MockProvider {
    fn returning(json: &str) -> Arc<Self> {
        Arc::new(Self {
            response: std::sync::Mutex::new(Some(json.to_string())),
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

#[async_trait]
impl Provider for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }
    async fn stream_chat(&self, _p: &ChatParams) -> Result<ChatStream, LoopalError> {
        let text = self.response.lock().unwrap().take().unwrap();
        let chunks = VecDeque::from(vec![
            Ok(StreamChunk::Text { text }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ]);
        Ok(Box::pin(MockStream(chunks)))
    }
}

struct RecordingHandler {
    called: Arc<AtomicBool>,
    decision: PermissionDecision,
}

#[async_trait]
impl PermissionHandler for RecordingHandler {
    async fn decide(
        &self,
        _id: &str,
        _name: &str,
        _input: &serde_json::Value,
    ) -> PermissionOutcome {
        self.called.store(true, Ordering::SeqCst);
        PermissionOutcome {
            decision: self.decision,
            reason: "mock".into(),
            duration_ms: 0,
        }
    }
}

#[tokio::test]
async fn falls_back_when_resolver_fails() {
    let classifier = Arc::new(AutoClassifier::new("".into()));
    let called = Arc::new(AtomicBool::new(false));
    let fallback = RecordingHandler {
        called: called.clone(),
        decision: PermissionDecision::Deny,
    };
    let auto = AutoPermissionHandler::new(
        classifier,
        Box::new(fallback),
        Arc::new(FailingResolver),
        DecisionContext::with_cwd("/tmp/test"),
    );
    let outcome = auto.decide("id1", "Bash", &serde_json::json!({})).await;
    assert_eq!(outcome.decision, PermissionDecision::Deny);
    assert!(called.load(Ordering::SeqCst), "fallback must be called");
    assert!(
        outcome.reason.contains("provider lookup failed") || outcome.reason == "mock",
        "fallback reason expected: {}",
        outcome.reason
    );
}

#[tokio::test]
async fn fallback_allow_decision_propagates_through() {
    let classifier = Arc::new(AutoClassifier::new("".into()));
    let fallback = RecordingHandler {
        called: Arc::new(AtomicBool::new(false)),
        decision: PermissionDecision::Allow,
    };
    let auto = AutoPermissionHandler::new(
        classifier,
        Box::new(fallback),
        Arc::new(FailingResolver),
        DecisionContext::with_cwd("/tmp/test"),
    );
    let outcome = auto.decide("id2", "Read", &serde_json::json!({})).await;
    assert_eq!(outcome.decision, PermissionDecision::Allow);
}

#[tokio::test]
async fn context_cell_round_trip() {
    let cell = DecisionContext::with_cwd("/tmp/proj");
    cell.set_recent("recent: hello".into()).await;
    assert_eq!(cell.recent().await, "recent: hello");
    assert_eq!(cell.cwd(), "/tmp/proj");
}

#[tokio::test]
async fn provider_error_with_deny_action_does_not_call_fallback() {
    let classifier = Arc::new(AutoClassifier::new("".into()));
    let fb_called = Arc::new(AtomicBool::new(false));
    let fallback = RecordingHandler {
        called: fb_called.clone(),
        decision: PermissionDecision::Allow,
    };
    let auto = AutoPermissionHandler::new(
        classifier,
        Box::new(fallback),
        Arc::new(FailingResolver),
        DecisionContext::with_cwd("/tmp/test"),
    )
    .with_provider_error_action(DegradedAction::Deny);
    let outcome = auto.decide("id3", "Bash", &serde_json::json!({})).await;
    assert_eq!(
        outcome.decision,
        PermissionDecision::Deny,
        "Deny action should bypass the fallback handler"
    );
    assert!(
        !fb_called.load(Ordering::SeqCst),
        "fallback must not be called when DegradedAction::Deny is set"
    );
    assert!(
        outcome.reason.contains("provider lookup failed"),
        "Deny outcome should preserve the trigger reason: {}",
        outcome.reason
    );
}

#[tokio::test]
async fn degraded_classifier_skips_provider_and_falls_back() {
    let classifier = Arc::new(AutoClassifier::new("".into()));
    classifier.force_degraded_for_test("Bash");
    assert!(
        classifier.is_degraded(),
        "precondition: classifier degraded"
    );

    let called = Arc::new(AtomicBool::new(false));
    let fallback = RecordingHandler {
        called: called.clone(),
        decision: PermissionDecision::Deny,
    };
    let auto = AutoPermissionHandler::new(
        classifier,
        Box::new(fallback),
        Arc::new(FailingResolver),
        DecisionContext::with_cwd("/tmp/test"),
    );
    let outcome = auto.decide("id4", "Bash", &serde_json::json!({})).await;
    assert!(
        called.load(Ordering::SeqCst),
        "fallback must be called when classifier is degraded"
    );
    assert!(
        outcome.reason.contains("classifier degraded"),
        "reason should record the trigger (classifier degraded), got: {}",
        outcome.reason
    );
    assert!(
        outcome.reason.contains("fallback:"),
        "reason should chain the fallback's reason, got: {}",
        outcome.reason
    );
}

#[tokio::test]
async fn happy_path_classifier_allow_returns_allow_without_fallback() {
    let classifier = Arc::new(AutoClassifier::new("".into()));
    let provider =
        MockProvider::returning(r#"{"should_block": false, "reason": "normal cargo test"}"#);
    let resolver = Arc::new(MockResolver {
        provider,
        model: "claude-haiku".into(),
    });
    let fb_called = Arc::new(AtomicBool::new(false));
    let fallback = RecordingHandler {
        called: fb_called.clone(),
        decision: PermissionDecision::Deny,
    };
    let auto = AutoPermissionHandler::new(
        classifier,
        Box::new(fallback),
        resolver,
        DecisionContext::with_cwd("/tmp/test"),
    );
    let outcome = auto
        .decide(
            "id-happy-allow",
            "Bash",
            &serde_json::json!({"command": "cargo test"}),
        )
        .await;
    assert_eq!(
        outcome.decision,
        PermissionDecision::Allow,
        "classifier said allow → handler must return Allow"
    );
    assert!(
        !fb_called.load(Ordering::SeqCst),
        "fallback must NOT be called when classifier succeeds"
    );
    assert!(
        outcome.reason.contains("normal cargo test"),
        "reason must be the classifier's reason, got: {}",
        outcome.reason
    );
}

#[tokio::test]
async fn happy_path_classifier_block_returns_deny_without_fallback() {
    let classifier = Arc::new(AutoClassifier::new("".into()));
    let provider =
        MockProvider::returning(r#"{"should_block": true, "reason": "deletes data outside cwd"}"#);
    let resolver = Arc::new(MockResolver {
        provider,
        model: "claude-haiku".into(),
    });
    let fb_called = Arc::new(AtomicBool::new(false));
    let fallback = RecordingHandler {
        called: fb_called.clone(),
        decision: PermissionDecision::Allow,
    };
    let auto = AutoPermissionHandler::new(
        classifier,
        Box::new(fallback),
        resolver,
        DecisionContext::with_cwd("/tmp/test"),
    );
    let outcome = auto
        .decide(
            "id-happy-block",
            "Bash",
            &serde_json::json!({"command": "rm -rf /"}),
        )
        .await;
    assert_eq!(
        outcome.decision,
        PermissionDecision::Deny,
        "classifier said block → handler must return Deny"
    );
    assert!(
        !fb_called.load(Ordering::SeqCst),
        "fallback must NOT be called when classifier returns a definitive decision"
    );
    assert!(
        outcome.reason.contains("deletes data"),
        "reason must be the classifier's reason, got: {}",
        outcome.reason
    );
}
