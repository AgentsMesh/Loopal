use std::sync::Arc;

use indexmap::IndexMap;
use loopal_agent_server::testing::{
    SessionRef, SharedSession, build_model_router, build_session_handlers,
};
use loopal_config::{ResolvedConfig, Settings};
use loopal_decision_api::DecisionMode;
use loopal_kernel::Kernel;
use loopal_provider_api::SharedModelRouter;
use loopal_runtime::frontend::DecisionContext;

use super::permission_request_support::permission_request;

fn config() -> ResolvedConfig {
    ResolvedConfig {
        settings: Settings {
            decision_mode: DecisionMode::Agent,
            ..Settings::default()
        },
        workflow_preset_thinking_recommendation: None,
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

fn session() -> SessionRef {
    let (input_tx, _input_rx) = tokio::sync::mpsc::channel(1);
    let (watch_tx, _watch_rx) = tokio::sync::watch::channel(0u64);
    Arc::new(tokio::sync::RwLock::new(Arc::new(
        SharedSession::placeholder(
            input_tx,
            loopal_protocol::InterruptSignal::new(),
            Arc::new(watch_tx),
        ),
    )))
}

#[tokio::test]
async fn agent_decision_falls_back_to_classifier_path_today() {
    let config = config();
    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
    let router = SharedModelRouter::new(build_model_router(&config.settings)).reader();
    let (permission, question, _) = build_session_handlers(
        &config,
        &kernel,
        session(),
        DecisionContext::with_cwd("/tmp/test"),
        router,
    );

    let outcome = permission
        .decide(&permission_request("id1", "Bash", serde_json::json!({})))
        .await;
    assert_eq!(
        outcome.decision,
        loopal_tool_api::PermissionDecision::Deny,
        "Agent mode must fall through to Classifier permission path"
    );
    let outcome = question.ask(vec![]).await;
    assert!(matches!(
        outcome.response,
        loopal_protocol::UserQuestionResponse::Cancelled { .. }
    ));
}
