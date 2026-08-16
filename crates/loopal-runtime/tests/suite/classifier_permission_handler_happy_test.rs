use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use loopal_classifier::ClassifierEngine;
use loopal_runtime::frontend::permission_handler::{PermissionHandler, PermissionOutcome};
use loopal_runtime::frontend::{ClassifierPermissionHandler, DecisionContext};
use loopal_tool_api::PermissionDecision;
use serde_json::Value;

use super::classifier_permission_handler_support::{MockProvider, MockResolver, RecordingHandler};
use super::permission_request_support::permission_request;

async fn decide(handler: &impl PermissionHandler, id: &str, input: Value) -> PermissionOutcome {
    handler.decide(&permission_request(id, "Bash", input)).await
}

#[tokio::test]
async fn classifier_allow_returns_allow_without_fallback() {
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
    let outcome = decide(
        &auto,
        "id-happy-allow",
        serde_json::json!({"command": "cargo test"}),
    )
    .await;
    assert_eq!(outcome.decision, PermissionDecision::Allow);
    assert!(!fb_called.load(Ordering::SeqCst));
    assert!(outcome.reason.contains("normal cargo test"));
}

#[tokio::test]
async fn classifier_block_returns_deny_without_fallback() {
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
    let outcome = decide(
        &auto,
        "id-happy-block",
        serde_json::json!({"command": "rm -rf /"}),
    )
    .await;
    assert_eq!(outcome.decision, PermissionDecision::Deny);
    assert!(!fb_called.load(Ordering::SeqCst));
    assert!(outcome.reason.contains("deletes data"));
}
