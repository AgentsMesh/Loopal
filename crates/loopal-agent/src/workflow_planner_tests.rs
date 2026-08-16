use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use loopal_config::{OrchestrationPolicy, WorkflowSettings};
use loopal_protocol::WorkflowExecution;
use loopal_tool_api::{OneShotChatEffort, OneShotChatError, OneShotChatService};

use super::{ProactiveWorkflowPlanner, WorkflowPlanner, WorkflowPlannerOptions};

struct Reply(Result<String, OneShotChatError>);

#[async_trait]
impl OneShotChatService for Reply {
    async fn one_shot_chat(
        &self,
        _model: &str,
        _system_prompt: &str,
        _user_prompt: &str,
        _max_tokens: u32,
    ) -> Result<String, OneShotChatError> {
        self.0.clone()
    }
}

struct RecordingReply {
    response: String,
    model: Arc<Mutex<Option<String>>>,
    user_prompt: Arc<Mutex<Option<String>>>,
    effort: Arc<Mutex<Option<OneShotChatEffort>>>,
}

#[async_trait]
impl OneShotChatService for RecordingReply {
    async fn one_shot_chat(
        &self,
        model: &str,
        _system_prompt: &str,
        user_prompt: &str,
        _max_tokens: u32,
    ) -> Result<String, OneShotChatError> {
        *self.model.lock().unwrap() = Some(model.into());
        *self.user_prompt.lock().unwrap() = Some(user_prompt.into());
        Ok(self.response.clone())
    }

    async fn one_shot_chat_with_effort(
        &self,
        model: &str,
        _system_prompt: &str,
        user_prompt: &str,
        _max_tokens: u32,
        effort: OneShotChatEffort,
    ) -> Result<String, OneShotChatError> {
        *self.model.lock().unwrap() = Some(model.into());
        *self.user_prompt.lock().unwrap() = Some(user_prompt.into());
        *self.effort.lock().unwrap() = Some(effort);
        Ok(self.response.clone())
    }
}

fn planner(reply: Result<String, OneShotChatError>) -> ProactiveWorkflowPlanner {
    let settings = WorkflowSettings {
        execution_enabled: true,
        policy: OrchestrationPolicy::Proactive,
        ..Default::default()
    };
    ProactiveWorkflowPlanner::new(settings, Arc::new(Reply(reply)), "planner-model")
}

fn direct_reason(decision: loopal_protocol::WorkflowPlanDecision) -> String {
    let WorkflowExecution::Direct { reason } = decision.execution else {
        panic!("planner failure must fall back to direct execution")
    };
    reason.expect("direct fallback explains its reason")
}

fn workflow_goal() -> &'static str {
    "Ask multiple agents to independently cross-check separate implementations in parallel, then join their findings into one verified result"
}

#[tokio::test]
async fn provider_failure_falls_back_to_direct_without_external_effect() {
    let reason = direct_reason(
        planner(Err(OneShotChatError::ProviderUnresolvable))
            .plan(workflow_goal(), "")
            .await,
    );

    assert!(reason.contains("planner unavailable"), "{reason}");
}

#[tokio::test]
async fn invalid_planner_json_falls_back_to_direct() {
    let reason = direct_reason(
        planner(Ok("not a workflow plan".into()))
            .plan(workflow_goal(), "")
            .await,
    );

    assert!(reason.contains("planner output rejected"), "{reason}");
}

#[tokio::test]
async fn short_non_ascii_goal_remains_planner_driven() {
    let reason = direct_reason(
        planner(Ok("not a workflow plan".into()))
            .plan(
                "\u{8bf7}\u{8ba9}\u{591a}\u{4e2a}\u{667a}\u{80fd}\u{4f53}\u{5206}\u{522b}\u{5ba1}\u{67e5}\u{5e76}\u{4ea4}\u{53c9}\u{9a8c}\u{8bc1}\u{8fd9}\u{4e2a}\u{4fee}\u{590d}",
                "",
            )
            .await,
    );

    assert!(reason.contains("planner output rejected"), "{reason}");
}

#[tokio::test]
async fn invalid_workflow_candidate_falls_back_to_direct() {
    let invalid = serde_json::to_string(&loopal_protocol::WorkflowPlanDecision {
        version: loopal_protocol::WORKFLOW_PLAN_V1,
        execution: WorkflowExecution::Workflow {
            spec: loopal_protocol::WorkflowSpec {
                version: loopal_protocol::WORKFLOW_SPEC_V1,
                run_goal: "review".into(),
                nodes: Vec::new(),
                limits: loopal_protocol::WorkflowLimits {
                    max_nodes: 1,
                    max_parallel: 1,
                    max_attempts: 1,
                    run_deadline_ms: 1_000,
                    attempt_timeout_ms: 1_000,
                    max_output_bytes: 1_024,
                },
                output_node: "missing".into(),
                output_contract: loopal_protocol::WorkflowOutputContract::Text { max_bytes: 1_024 },
            },
        },
    })
    .unwrap();
    let reason = direct_reason(planner(Ok(invalid)).plan(workflow_goal(), "").await);

    assert!(reason.contains("workflow candidate rejected"), "{reason}");
}

#[tokio::test]
async fn disabled_trait_entrypoint_and_direct_decision_keep_contract() {
    let disabled = ProactiveWorkflowPlanner::new_with_options(
        WorkflowSettings::default(),
        Arc::new(Reply(Ok(String::new()))),
        "planner-model",
        WorkflowPlannerOptions::default(),
    );
    assert!(!disabled.settings().execution_enabled);
    assert!(ProactiveWorkflowPlanner::response_schema().is_object());
    let reason = direct_reason(WorkflowPlanner::plan(&disabled, workflow_goal(), "").await);
    assert!(reason.contains("disabled"));

    let direct = serde_json::json!({
        "version": 1,
        "execution": {
            "kind": "direct",
            "reason": "the provider selected direct execution"
        }
    })
    .to_string();
    let decision = planner(Ok(direct)).plan(workflow_goal(), "").await;
    assert_eq!(
        direct_reason(decision),
        "the provider selected direct execution"
    );
}

#[path = "workflow_planner_profile_tests.rs"]
mod profile_tests;
