use loopal_protocol::{
    AgentStateSnapshot, AgentStatus, ObservableAgentState, WorkflowNodeId, WorkflowRunId,
    WorkflowRunState, WorkflowRunSummary, WorkflowStateCounts,
};

#[test]
fn test_agent_status_default_is_starting() {
    assert_eq!(AgentStatus::default(), AgentStatus::Starting);
}

#[test]
fn test_agent_status_all_variants_debug() {
    let variants = [
        AgentStatus::Starting,
        AgentStatus::Running,
        AgentStatus::WaitingForInput,
        AgentStatus::Suspended,
        AgentStatus::Finished,
        AgentStatus::Error,
    ];
    for v in &variants {
        let debug = format!("{v:?}");
        assert!(!debug.is_empty());
    }
}

#[test]
fn test_agent_status_serde_roundtrip() {
    for status in [
        AgentStatus::Starting,
        AgentStatus::Running,
        AgentStatus::WaitingForInput,
        AgentStatus::Suspended,
        AgentStatus::Finished,
        AgentStatus::Error,
    ] {
        let json = serde_json::to_string(&status).unwrap();
        let restored: AgentStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, status);
    }
}

#[test]
fn test_observable_agent_state_default() {
    let state = ObservableAgentState::default();
    assert_eq!(state.status, AgentStatus::Starting);
    assert_eq!(state.turn_count, 0);
    assert_eq!(state.input_tokens, 0);
    assert_eq!(state.output_tokens, 0);
    assert!(state.model.is_empty());
    assert_eq!(state.thinking_config, "auto");
    assert_eq!(state.mode, "act");
}

#[test]
fn test_observable_agent_state_serde_roundtrip() {
    let state = ObservableAgentState {
        status: AgentStatus::Running,
        turn_count: 3,
        input_tokens: 1000,
        output_tokens: 500,
        model: "claude-sonnet".to_string(),
        thinking_config: "effort".to_string(),
        mode: "plan".to_string(),
        permission_mode: "bypass".to_string(),
        decision_mode: "manual".to_string(),
        sandbox_policy: "default_write".to_string(),
    };
    let json = serde_json::to_string(&state).unwrap();
    let restored: ObservableAgentState = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.status, AgentStatus::Running);
    assert_eq!(restored.turn_count, 3);
    assert_eq!(restored.thinking_config, "effort");
    assert_eq!(restored.permission_mode, "bypass");
    assert_eq!(restored.decision_mode, "manual");
    assert_eq!(restored.sandbox_policy, "default_write");
}

#[test]
fn test_observable_thinking_config_defaults_when_absent() {
    let legacy_json = r#"{
        "status": "Running",
        "turn_count": 0,
        "input_tokens": 0,
        "output_tokens": 0,
        "model": "claude-x",
        "mode": "act"
    }"#;
    let restored: ObservableAgentState = serde_json::from_str(legacy_json).unwrap();
    assert_eq!(restored.thinking_config, "auto");
}

fn workflow_summary() -> WorkflowRunSummary {
    WorkflowRunSummary {
        id: WorkflowRunId::new("run-1"),
        run_goal: "ship".into(),
        state: WorkflowRunState::Running,
        revision: 4,
        output_node: WorkflowNodeId::new("output"),
        counts: WorkflowStateCounts {
            pending: 0,
            ready: 1,
            active: 0,
            succeeded: 0,
            failed: 0,
            cancelled: 0,
            skipped: 0,
        },
        created_at_unix_ms: 10,
        updated_at_unix_ms: 20,
    }
}

#[test]
fn agent_snapshot_legacy_wire_defaults_workflows() {
    let legacy = r#"{"tasks":[],"crons":[],"bg_tasks":[]}"#;
    let snapshot: AgentStateSnapshot = serde_json::from_str(legacy).unwrap();
    assert!(snapshot.workflows.is_empty());

    let partial = r#"{"tasks":[],"crons":[],"bg_tasks":[],"workflows":{"active":[]}}"#;
    let snapshot: AgentStateSnapshot = serde_json::from_str(partial).unwrap();
    assert!(snapshot.workflows.recent.is_empty());

    let encoded = serde_json::to_value(AgentStateSnapshot::empty()).unwrap();
    assert!(encoded.get("workflows").is_none());
}

#[test]
fn agent_snapshot_roundtrips_workflow_summaries() {
    let mut snapshot = AgentStateSnapshot::empty();
    snapshot.workflows.active.push(workflow_summary());
    let encoded = serde_json::to_string(&snapshot).unwrap();
    let decoded: AgentStateSnapshot = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.workflows, snapshot.workflows);
}
