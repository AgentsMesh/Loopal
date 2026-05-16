use indexmap::IndexMap;
use std::sync::Arc;

use loopal_agent_server::testing::{SessionRef, SharedSession, build_session_handlers};
use loopal_config::{ResolvedConfig, Settings};
use loopal_decision_api::DecisionMode;
use loopal_kernel::Kernel;
use loopal_runtime::frontend::DecisionContext;

fn empty_config(decision: DecisionMode) -> ResolvedConfig {
    let settings = Settings {
        decision_mode: decision,
        ..Settings::default()
    };
    ResolvedConfig {
        settings,
        mcp_servers: IndexMap::new(),
        skills: IndexMap::new(),
        hooks: Vec::new(),
        instructions: String::new(),
        memory: String::new(),
        classifier_prompt: None,
        layers: Vec::new(),
        secrets: None,
    }
}

fn dummy_session() -> SessionRef {
    let (input_tx, _input_rx) = tokio::sync::mpsc::channel(1);
    let interrupt = loopal_protocol::InterruptSignal::new();
    let (watch_tx, _watch_rx) = tokio::sync::watch::channel(0u64);
    Arc::new(tokio::sync::RwLock::new(Arc::new(
        SharedSession::placeholder(input_tx, interrupt, Arc::new(watch_tx)),
    )))
}

#[tokio::test]
async fn manual_decision_yields_ipc_only_no_primary_connection() {
    let config = empty_config(DecisionMode::Manual);
    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
    let session = dummy_session();
    let (perm, _q) = build_session_handlers(
        &config,
        &kernel,
        session,
        DecisionContext::with_cwd("/tmp/test"),
    );
    let outcome = perm.decide("id1", "Bash", &serde_json::json!({})).await;
    assert_eq!(
        outcome.decision,
        loopal_tool_api::PermissionDecision::Deny,
        "no primary connection should fail closed"
    );
    assert!(
        outcome.reason.contains("no primary connection"),
        "Manual path reason should reflect ipc state, got: {}",
        outcome.reason
    );
}

#[tokio::test]
async fn auto_decision_wraps_with_auto_handlers_and_falls_back() {
    let config = empty_config(DecisionMode::Classifier);
    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
    let session = dummy_session();
    let (perm, _q) = build_session_handlers(
        &config,
        &kernel,
        session,
        DecisionContext::with_cwd("/tmp/test"),
    );
    let outcome = perm.decide("id1", "Bash", &serde_json::json!({})).await;
    assert_eq!(
        outcome.decision,
        loopal_tool_api::PermissionDecision::Deny,
        "auto -> fallback ipc -> no connection should still deny"
    );
    assert!(
        outcome.reason.contains("provider lookup failed"),
        "Classifier path should record the trigger reason, got: {}",
        outcome.reason
    );
    assert!(
        outcome.reason.contains("fallback:"),
        "Classifier path should chain the fallback's reason after the trigger, got: {}",
        outcome.reason
    );
}

#[tokio::test]
async fn manual_question_path_cancels_without_connection() {
    let config = empty_config(DecisionMode::Manual);
    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
    let session = dummy_session();
    let (_perm, q) = build_session_handlers(
        &config,
        &kernel,
        session,
        DecisionContext::with_cwd("/tmp/test"),
    );
    let outcome = q.ask(vec![]).await;
    assert!(
        matches!(
            outcome.response,
            loopal_protocol::UserQuestionResponse::Cancelled { .. }
        ),
        "no primary connection should cancel"
    );
    assert!(
        outcome.reason.contains("no primary connection"),
        "Manual question path should reflect ipc state, got: {}",
        outcome.reason
    );
}

#[tokio::test]
async fn auto_question_path_chains_fallback_when_provider_unresolvable() {
    let config = empty_config(DecisionMode::Classifier);
    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
    let session = dummy_session();
    let (_perm, q) = build_session_handlers(
        &config,
        &kernel,
        session,
        DecisionContext::with_cwd("/tmp/test"),
    );
    let outcome = q.ask(vec![]).await;
    assert!(
        matches!(
            outcome.response,
            loopal_protocol::UserQuestionResponse::Cancelled { .. }
        ),
        "auto -> fallback ipc -> no connection should cancel"
    );
    assert!(
        outcome.reason.contains("no primary connection"),
        "Classifier path with no provider should short-circuit to manual fallback, got: {}",
        outcome.reason
    );
}

#[tokio::test]
async fn agent_decision_falls_back_to_classifier_path_today() {
    // Agent mode is not yet implemented; factory must transparently fall
    // back to Classifier behaviour so existing setups keep working.
    let config = empty_config(DecisionMode::Agent);
    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
    let session = dummy_session();
    let (perm, q) = build_session_handlers(
        &config,
        &kernel,
        session,
        DecisionContext::with_cwd("/tmp/test"),
    );
    // Permission path: Agent → Classifier → IpcPermission fallback denies (no conn)
    let outcome = perm.decide("id1", "Bash", &serde_json::json!({})).await;
    assert_eq!(
        outcome.decision,
        loopal_tool_api::PermissionDecision::Deny,
        "Agent mode must fall through to Classifier permission path"
    );
    // Question path: Agent → Classifier → IpcQuestion fallback cancels (no conn)
    let q_outcome = q.ask(vec![]).await;
    assert!(
        matches!(
            q_outcome.response,
            loopal_protocol::UserQuestionResponse::Cancelled { .. }
        ),
        "Agent mode question path should reach the manual fallback (no conn)"
    );
}
