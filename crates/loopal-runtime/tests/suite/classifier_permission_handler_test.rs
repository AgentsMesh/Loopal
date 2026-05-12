use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use loopal_classifier::ClassifierEngine;
use loopal_runtime::frontend::permission_handler::PermissionHandler;
use loopal_runtime::frontend::{ClassifierPermissionHandler, DecisionContext, DegradedAction};
use loopal_tool_api::PermissionDecision;

use super::classifier_permission_handler_support::{
    FailingResolver, MockProvider, MockResolver, RecordingHandler,
};

#[tokio::test]
async fn falls_back_when_resolver_fails() {
    let classifier = Arc::new(ClassifierEngine::new("".into()));
    let called = Arc::new(AtomicBool::new(false));
    let fallback = RecordingHandler {
        called: called.clone(),
        decision: PermissionDecision::Deny,
    };
    let auto = ClassifierPermissionHandler::new(
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
    let classifier = Arc::new(ClassifierEngine::new("".into()));
    let fallback = RecordingHandler {
        called: Arc::new(AtomicBool::new(false)),
        decision: PermissionDecision::Allow,
    };
    let auto = ClassifierPermissionHandler::new(
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
    let classifier = Arc::new(ClassifierEngine::new("".into()));
    let fb_called = Arc::new(AtomicBool::new(false));
    let fallback = RecordingHandler {
        called: fb_called.clone(),
        decision: PermissionDecision::Allow,
    };
    let auto = ClassifierPermissionHandler::new(
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
    let classifier = Arc::new(ClassifierEngine::new("".into()));
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
    let auto = ClassifierPermissionHandler::new(
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
async fn manual_allow_after_degraded_resets_circuit_breaker() {
    // Wire test: when the fallback path returns Allow on a degraded classifier,
    // on_human_approval must fire so the classifier exits degraded state.
    let classifier = Arc::new(ClassifierEngine::new("".into()));
    classifier.force_degraded_for_test("Bash");
    assert!(classifier.is_degraded(), "precondition");

    let fallback = RecordingHandler {
        called: Arc::new(AtomicBool::new(false)),
        decision: PermissionDecision::Allow,
    };
    let auto = ClassifierPermissionHandler::new(
        classifier.clone(),
        Box::new(fallback),
        Arc::new(FailingResolver),
        DecisionContext::with_cwd("/tmp/test"),
    );
    let outcome = auto
        .decide("id-allow", "Bash", &serde_json::json!({}))
        .await;
    assert_eq!(outcome.decision, PermissionDecision::Allow);
    assert!(
        !classifier.is_degraded(),
        "user Allow on fallback must reset degraded — classifier should be ready to try again"
    );
}

#[tokio::test]
async fn manual_deny_after_degraded_keeps_circuit_breaker_degraded() {
    // Counter-test: Deny on fallback must NOT reset the circuit; persistent
    // user denial means the classifier stays sidelined.
    let classifier = Arc::new(ClassifierEngine::new("".into()));
    classifier.force_degraded_for_test("Bash");

    let fallback = RecordingHandler {
        called: Arc::new(AtomicBool::new(false)),
        decision: PermissionDecision::Deny,
    };
    let auto = ClassifierPermissionHandler::new(
        classifier.clone(),
        Box::new(fallback),
        Arc::new(FailingResolver),
        DecisionContext::with_cwd("/tmp/test"),
    );
    let _ = auto.decide("id-deny", "Bash", &serde_json::json!({})).await;
    assert!(
        classifier.is_degraded(),
        "Deny on fallback should not reset degraded state"
    );
}

#[tokio::test]
async fn happy_path_classifier_allow_returns_allow_without_fallback() {
    let classifier = Arc::new(ClassifierEngine::new("".into()));
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
    let auto = ClassifierPermissionHandler::new(
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
    assert_eq!(outcome.decision, PermissionDecision::Allow);
    assert!(!fb_called.load(Ordering::SeqCst));
    assert!(outcome.reason.contains("normal cargo test"));
}

#[tokio::test]
async fn happy_path_classifier_block_returns_deny_without_fallback() {
    let classifier = Arc::new(ClassifierEngine::new("".into()));
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
    let auto = ClassifierPermissionHandler::new(
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
    assert_eq!(outcome.decision, PermissionDecision::Deny);
    assert!(!fb_called.load(Ordering::SeqCst));
    assert!(outcome.reason.contains("deletes data"));
}
