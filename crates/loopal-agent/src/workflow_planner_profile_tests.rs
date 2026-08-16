use std::sync::{Arc, Mutex};

use loopal_config::{OrchestrationPolicy, WorkflowSettings};
use loopal_protocol::{WorkflowExecution, workflow_plan_schema};
use loopal_tool_api::OneShotChatEffort;

use super::{ProactiveWorkflowPlanner, RecordingReply, WorkflowPlannerOptions, workflow_goal};

#[tokio::test]
async fn production_prompt_contains_canonical_schema_and_trusted_ceilings() {
    let user_prompt = Arc::new(Mutex::new(None));
    let response = serde_json::json!({
        "version": 1,
        "execution": {"kind": "workflow", "spec": {
            "version": 1,
            "run_goal": "review",
            "nodes": [{"id": "node", "dependencies": [], "task": "work", "worker_profile": "default"}],
            "limits": {"max_nodes": 1, "max_parallel": 1, "max_attempts": 1, "run_deadline_ms": 9000, "attempt_timeout_ms": 4000, "max_output_bytes": 1024},
            "output_node": "node",
            "output_contract": {"type": "text", "max_bytes": 1024}
        }}
    })
    .to_string();
    let mut settings = WorkflowSettings {
        execution_enabled: true,
        policy: OrchestrationPolicy::Proactive,
        ..Default::default()
    };
    settings.limits.max_nodes = 3;
    settings.limits.max_parallel = 2;
    settings.limits.max_attempts = 3;
    settings.limits.max_output_bytes = 2_048;
    settings.timing.run_deadline_secs = 9;
    settings.timing.attempt_timeout_secs = 4;
    let planner = ProactiveWorkflowPlanner::new(
        settings,
        Arc::new(RecordingReply {
            response,
            model: Arc::new(Mutex::new(None)),
            user_prompt: user_prompt.clone(),
            effort: Arc::new(Mutex::new(None)),
        }),
        "planner-model",
    );

    let decision = planner.plan(workflow_goal(), "recent context").await;
    assert!(matches!(
        decision.execution,
        WorkflowExecution::Workflow { .. }
    ));
    let prompt = user_prompt
        .lock()
        .unwrap()
        .clone()
        .expect("prompt captured");
    let schema = prompt
        .split_once("<schema>\n")
        .and_then(|(_, value)| value.split_once("\n</schema>"))
        .map(|(value, _)| serde_json::from_str::<serde_json::Value>(value).unwrap())
        .expect("canonical schema is present in planner prompt");
    assert_eq!(schema, workflow_plan_schema());
    assert!(prompt.contains("\"max_nodes\":3"));
    assert!(prompt.contains("\"run_deadline_ms\":9000"));
    assert!(prompt.contains("worker_profile must be one of default, explore, or plan"));
}

#[tokio::test]
async fn production_prompt_caps_goal_at_protocol_byte_limit_on_utf8_boundary() {
    let user_prompt = Arc::new(Mutex::new(None));
    let planner = ProactiveWorkflowPlanner::new(
        WorkflowSettings {
            execution_enabled: true,
            policy: OrchestrationPolicy::Proactive,
            ..Default::default()
        },
        Arc::new(RecordingReply {
            response: "not json".into(),
            model: Arc::new(Mutex::new(None)),
            user_prompt: user_prompt.clone(),
            effort: Arc::new(Mutex::new(None)),
        }),
        "planner-model",
    );
    let goal = format!(
        "{}TAIL",
        "\u{754c}".repeat(loopal_protocol::MAX_WORKFLOW_GOAL_BYTES)
    );

    let _ = planner.plan(&goal, "").await;
    let prompt = user_prompt.lock().unwrap().clone().unwrap();
    let task = prompt
        .split_once("<task>\n")
        .and_then(|(_, value)| value.split_once("\n</task>"))
        .map(|(value, _)| value)
        .expect("bounded task is present");
    assert!(task.len() <= loopal_protocol::MAX_WORKFLOW_GOAL_BYTES);
    assert!(!task.contains("TAIL"));
    assert!(task.chars().all(|character| character == '\u{754c}'));
}

#[tokio::test]
async fn ultracode_profile_requests_max_one_shot_effort() {
    let effort = Arc::new(Mutex::new(None));
    let settings = WorkflowSettings {
        execution_enabled: true,
        policy: OrchestrationPolicy::Proactive,
        planner_profile: loopal_config::WorkflowPlannerProfile::Ultracode,
        ..Default::default()
    };
    let planner = ProactiveWorkflowPlanner::new(
        settings,
        Arc::new(RecordingReply {
            response: "not json".into(),
            model: Arc::new(Mutex::new(None)),
            user_prompt: Arc::new(Mutex::new(None)),
            effort: effort.clone(),
        }),
        "planner-model",
    );

    let _ = planner.plan(workflow_goal(), "").await;
    assert_eq!(*effort.lock().unwrap(), Some(OneShotChatEffort::Max));
}

#[tokio::test]
async fn ultracode_preserves_an_explicit_max_effort_recommendation() {
    let effort = Arc::new(Mutex::new(None));
    let planner = ProactiveWorkflowPlanner::new_with_options(
        WorkflowSettings {
            execution_enabled: true,
            policy: OrchestrationPolicy::Proactive,
            planner_profile: loopal_config::WorkflowPlannerProfile::Ultracode,
            ..Default::default()
        },
        Arc::new(RecordingReply {
            response: "not json".into(),
            model: Arc::new(Mutex::new(None)),
            user_prompt: Arc::new(Mutex::new(None)),
            effort: effort.clone(),
        }),
        "planner-model",
        WorkflowPlannerOptions {
            recommended_effort: OneShotChatEffort::Max,
        },
    );

    let _ = planner.plan(workflow_goal(), "").await;
    assert_eq!(*effort.lock().unwrap(), Some(OneShotChatEffort::Max));
}

#[tokio::test]
async fn shared_model_router_tracks_runtime_model_switches() {
    let model = Arc::new(Mutex::new(None));
    let router = loopal_provider_api::SharedModelRouter::with_default("model-a".into());
    let planner = ProactiveWorkflowPlanner::new_with_model_router(
        WorkflowSettings {
            execution_enabled: true,
            policy: OrchestrationPolicy::Proactive,
            ..Default::default()
        },
        Arc::new(RecordingReply {
            response: "not json".into(),
            model: model.clone(),
            user_prompt: Arc::new(Mutex::new(None)),
            effort: Arc::new(Mutex::new(None)),
        }),
        router.reader(),
    );

    let _ = planner.plan(workflow_goal(), "").await;
    assert_eq!(model.lock().unwrap().as_deref(), Some("model-a"));
    router.set_default("model-b".into());
    let _ = planner.plan(workflow_goal(), "").await;
    assert_eq!(model.lock().unwrap().as_deref(), Some("model-b"));
}
