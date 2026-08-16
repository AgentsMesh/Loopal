use loopal_config::{
    OrchestrationPolicy, WorkflowLimits, WorkflowPlannerProfile, WorkflowPreset, WorkflowSettings,
    WorkflowTiming,
};

#[test]
fn workflow_defaults_are_inert_and_safe() {
    let workflow = WorkflowSettings::default();
    assert_eq!(workflow.policy, OrchestrationPolicy::Off);
    assert!(!workflow.execution_enabled);
    assert_eq!(workflow.planner_profile, WorkflowPlannerProfile::Default);
    assert_eq!(workflow.preset, None);
    assert!(workflow.limits.max_nodes > 0);
    assert!(workflow.limits.max_parallel <= workflow.limits.max_nodes);
    assert!(workflow.limits.max_attempts >= workflow.limits.max_nodes);
    assert!(workflow.validate().is_ok());
}

#[test]
fn workflow_round_trip_covers_every_policy_and_preset() {
    for policy in [
        OrchestrationPolicy::Off,
        OrchestrationPolicy::Explicit,
        OrchestrationPolicy::Proactive,
    ] {
        let workflow = WorkflowSettings {
            policy,
            execution_enabled: true,
            planner_profile: WorkflowPlannerProfile::Ultracode,
            limits: WorkflowLimits {
                max_nodes: 40,
                max_parallel: 8,
                max_attempts: 80,
                max_output_bytes: 512_000,
            },
            timing: WorkflowTiming {
                run_deadline_secs: 7200,
                attempt_timeout_secs: 1200,
                cancel_grace_secs: 20,
                recovery_grace_secs: 90,
            },
            preset: Some(WorkflowPreset::Ultracode),
        };
        let json = serde_json::to_string(&workflow).unwrap();
        let decoded: WorkflowSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, workflow);
    }
}

#[test]
fn workflow_unknown_fields_are_ignored() {
    let workflow: WorkflowSettings = serde_json::from_value(serde_json::json!({
        "policy": "explicit",
        "future_top_level": true,
        "limits": {"max_nodes": 24, "future_limit": 99},
        "timing": {"cancel_grace_secs": 12, "future_timing": 99}
    }))
    .unwrap();
    assert_eq!(workflow.policy, OrchestrationPolicy::Explicit);
    assert_eq!(workflow.limits.max_nodes, 24);
    assert_eq!(workflow.timing.cancel_grace_secs, 12);
    assert!(!workflow.execution_enabled);
}

#[test]
fn sanitize_clamps_every_absolute_ceiling() {
    let workflow = WorkflowSettings {
        limits: WorkflowLimits {
            max_nodes: u32::MAX,
            max_parallel: u32::MAX,
            max_attempts: u32::MAX,
            max_output_bytes: u64::MAX,
        },
        timing: WorkflowTiming {
            run_deadline_secs: u64::MAX,
            attempt_timeout_secs: u64::MAX,
            cancel_grace_secs: u64::MAX,
            recovery_grace_secs: u64::MAX,
        },
        ..WorkflowSettings::default()
    };
    let (sanitized, warnings) = workflow.sanitize();
    assert_eq!(
        sanitized.limits.max_nodes,
        WorkflowLimits::ABSOLUTE_MAX_NODES
    );
    assert_eq!(
        sanitized.limits.max_parallel,
        WorkflowLimits::ABSOLUTE_MAX_PARALLEL
    );
    assert_eq!(
        sanitized.limits.max_attempts,
        WorkflowLimits::ABSOLUTE_MAX_ATTEMPTS
    );
    assert_eq!(
        sanitized.limits.max_output_bytes,
        WorkflowLimits::ABSOLUTE_MAX_OUTPUT_BYTES
    );
    assert_eq!(
        sanitized.timing.run_deadline_secs,
        WorkflowTiming::ABSOLUTE_MAX_RUN_DEADLINE_SECS
    );
    assert_eq!(
        sanitized.timing.attempt_timeout_secs,
        WorkflowTiming::ABSOLUTE_MAX_ATTEMPT_TIMEOUT_SECS
    );
    assert_eq!(
        sanitized.timing.cancel_grace_secs,
        WorkflowTiming::ABSOLUTE_MAX_CANCEL_GRACE_SECS
    );
    assert_eq!(
        sanitized.timing.recovery_grace_secs,
        WorkflowTiming::ABSOLUTE_MAX_RECOVERY_GRACE_SECS
    );
    assert_eq!(warnings.len(), 8);
}

#[test]
fn sanitize_enforces_graph_and_timeout_relationships() {
    let workflow = WorkflowSettings {
        limits: WorkflowLimits {
            max_nodes: 2,
            max_parallel: 9,
            ..WorkflowLimits::default()
        },
        timing: WorkflowTiming {
            run_deadline_secs: 10,
            attempt_timeout_secs: 20,
            ..WorkflowTiming::default()
        },
        ..WorkflowSettings::default()
    };
    let (sanitized, warnings) = workflow.sanitize();
    assert_eq!(sanitized.limits.max_parallel, 2);
    assert_eq!(sanitized.timing.attempt_timeout_secs, 10);
    assert_eq!(warnings.len(), 2);
}

#[test]
fn validation_rejects_zero_and_invalid_limits() {
    let mut workflow = WorkflowSettings::default();
    workflow.limits.max_nodes = 0;
    assert!(workflow.validate().unwrap_err().contains("max_nodes"));

    let mut workflow = WorkflowSettings::default();
    workflow.limits.max_attempts = workflow.limits.max_nodes - 1;
    assert!(workflow.validate().unwrap_err().contains("max_attempts"));
}

#[test]
fn planner_profile_is_a_closed_set() {
    assert!(serde_json::from_str::<WorkflowSettings>(r#"{"planner_profile":"default"}"#).is_ok());
    assert!(serde_json::from_str::<WorkflowSettings>(r#"{"planner_profile":"ultracode"}"#).is_ok());
    assert!(
        serde_json::from_str::<WorkflowSettings>(r#"{"planner_profile":"provider-name"}"#).is_err()
    );
}
