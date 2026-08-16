use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use loopal_classifier::ClassifierEngine;
use loopal_protocol::{
    PermissionIntentRequest, WorkflowAttemptId, WorkflowNodeId, WorkflowPermissionCausation,
    WorkflowRunId,
};
use loopal_runtime::frontend::permission_handler::{PermissionHandler, PermissionOutcome};
use loopal_runtime::frontend::{ClassifierPermissionHandler, DecisionContext, DegradedAction};
use loopal_tool_api::PermissionDecision;
use serde_json::Value;

use super::classifier_permission_handler_support::{FailingResolver, RecordingHandler};
use super::permission_request_support::permission_request;

async fn decide(
    handler: &impl PermissionHandler,
    id: &str,
    name: &str,
    input: Value,
) -> PermissionOutcome {
    handler.decide(&permission_request(id, name, input)).await
}

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
    let outcome = decide(&auto, "id1", "Bash", serde_json::json!({})).await;
    assert_eq!(outcome.decision, PermissionDecision::Deny);
    assert!(called.load(Ordering::SeqCst));
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
    let outcome = decide(&auto, "id2", "Read", serde_json::json!({})).await;
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
    let outcome = decide(&auto, "id3", "Bash", serde_json::json!({})).await;
    assert_eq!(outcome.decision, PermissionDecision::Deny);
    assert!(!fb_called.load(Ordering::SeqCst));
    assert!(outcome.reason.contains("provider lookup failed"));
}

#[tokio::test]
async fn workflow_request_always_delegates_to_fallback() {
    let classifier = Arc::new(ClassifierEngine::new("".into()));
    let called = Arc::new(AtomicBool::new(false));
    let fallback = RecordingHandler {
        called: called.clone(),
        decision: PermissionDecision::Allow,
    };
    let auto = ClassifierPermissionHandler::new(
        classifier,
        Box::new(fallback),
        Arc::new(FailingResolver),
        DecisionContext::with_cwd("/tmp/test"),
    )
    .with_provider_error_action(DegradedAction::Deny);
    let request = PermissionIntentRequest::create(
        "workflow-effect",
        "Bash",
        serde_json::json!({"command": "true"}),
        serde_json::json!({"command": "true"}),
        serde_json::json!({"type": "object"}),
        Some(WorkflowPermissionCausation {
            run_id: WorkflowRunId::new("wrun_classifier"),
            node_id: WorkflowNodeId::new("wnode_classifier"),
            attempt_id: WorkflowAttemptId::new("watt_classifier"),
        }),
    )
    .unwrap();

    let outcome = auto.decide(&request).await;

    assert_eq!(outcome.decision, PermissionDecision::Allow);
    assert!(called.load(Ordering::SeqCst));
}

#[tokio::test]
async fn degraded_classifier_skips_provider_and_falls_back() {
    let classifier = Arc::new(ClassifierEngine::new("".into()));
    classifier.force_degraded_for_test("Bash");
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
    let outcome = decide(&auto, "id4", "Bash", serde_json::json!({})).await;
    assert!(called.load(Ordering::SeqCst));
    assert!(outcome.reason.contains("classifier degraded"));
    assert!(outcome.reason.contains("fallback:"));
}

#[tokio::test]
async fn manual_allow_after_degraded_resets_circuit_breaker() {
    let classifier = Arc::new(ClassifierEngine::new("".into()));
    classifier.force_degraded_for_test("Bash");
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
    let outcome = decide(&auto, "id-allow", "Bash", serde_json::json!({})).await;
    assert_eq!(outcome.decision, PermissionDecision::Allow);
    assert!(!classifier.is_degraded());
}

#[tokio::test]
async fn manual_deny_after_degraded_keeps_circuit_breaker_degraded() {
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
    let _ = decide(&auto, "id-deny", "Bash", serde_json::json!({})).await;
    assert!(classifier.is_degraded());
}
