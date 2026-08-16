use loopal_config::loader::apply_env_overrides;
use loopal_config::{
    ConfigResolver, OrchestrationPolicy, WorkflowLimits, WorkflowPreset, WorkflowTiming,
};

#[test]
fn workflow_env_overrides_cover_policy_profile_limits_and_timing() {
    let env = [
        ("LOOPAL_WORKFLOW_POLICY", "explicit"),
        ("LOOPAL_WORKFLOW_PLANNER_PROFILE", "ultracode"),
        ("LOOPAL_WORKFLOW_MAX_NODES", "45"),
        ("LOOPAL_WORKFLOW_MAX_PARALLEL", "9"),
        ("LOOPAL_WORKFLOW_MAX_ATTEMPTS", "90"),
        ("LOOPAL_WORKFLOW_MAX_OUTPUT_BYTES", "500000"),
        ("LOOPAL_WORKFLOW_RUN_DEADLINE_SECS", "5000"),
        ("LOOPAL_WORKFLOW_ATTEMPT_TIMEOUT_SECS", "1000"),
        ("LOOPAL_WORKFLOW_CANCEL_GRACE_SECS", "25"),
        ("LOOPAL_WORKFLOW_RECOVERY_GRACE_SECS", "125"),
    ];
    let _guard = EnvGuard::set(&env);
    let mut value = serde_json::json!({"workflow": {"execution_enabled": false}});
    apply_env_overrides(&mut value);
    assert_eq!(value["workflow"]["policy"], "explicit");
    assert_eq!(value["workflow"]["planner_profile"], "ultracode");
    assert_eq!(value["workflow"]["limits"]["max_nodes"], 45);
    assert_eq!(value["workflow"]["limits"]["max_parallel"], 9);
    assert_eq!(value["workflow"]["limits"]["max_attempts"], 90);
    assert_eq!(value["workflow"]["limits"]["max_output_bytes"], 500000);
    assert_eq!(value["workflow"]["timing"]["run_deadline_secs"], 5000);
    assert_eq!(value["workflow"]["timing"]["attempt_timeout_secs"], 1000);
    assert_eq!(value["workflow"]["timing"]["cancel_grace_secs"], 25);
    assert_eq!(value["workflow"]["timing"]["recovery_grace_secs"], 125);
    assert_eq!(value["workflow"]["execution_enabled"], false);
}

#[test]
fn workflow_env_has_no_execution_or_preset_bypass() {
    let _guard = EnvGuard::set(&[
        ("LOOPAL_WORKFLOW_EXECUTION_ENABLED", "true"),
        ("LOOPAL_WORKFLOW_PRESET", "ultracode"),
    ]);
    let mut value = serde_json::json!({
        "workflow": {"execution_enabled": false, "preset": null}
    });
    apply_env_overrides(&mut value);
    assert_eq!(value["workflow"]["execution_enabled"], false);
    assert!(value["workflow"]["preset"].is_null());
}

#[test]
fn resolver_clamps_ceilings_and_keeps_preset_execution_latched() {
    let mut resolver = ConfigResolver::new();
    resolver.add_layer(loopal_config::ConfigLayer {
        source: loopal_config::LayerSource::Project,
        settings: serde_json::json!({
            "workflow": {
                "preset": "ultracode",
                "policy": "off",
                "limits": {
                    "max_nodes": u32::MAX,
                    "max_parallel": u32::MAX,
                    "max_attempts": u32::MAX,
                    "max_output_bytes": u64::MAX
                },
                "timing": {
                    "run_deadline_secs": u64::MAX,
                    "attempt_timeout_secs": u64::MAX,
                    "cancel_grace_secs": u64::MAX,
                    "recovery_grace_secs": u64::MAX
                }
            }
        }),
        ..Default::default()
    });
    let resolved = resolver.resolve().unwrap();
    assert!(matches!(
        resolved.workflow_preset_thinking_recommendation,
        Some(loopal_provider_api::ThinkingConfig::Effort {
            level: loopal_provider_api::EffortLevel::Max
        })
    ));
    let settings_json = serde_json::to_value(&resolved.settings).unwrap();
    assert!(
        settings_json
            .get("workflow_preset_thinking_recommendation")
            .is_none()
    );
    let settings = resolved.settings;
    assert_eq!(settings.workflow.policy, OrchestrationPolicy::Proactive);
    assert!(!settings.workflow.execution_enabled);
    assert_eq!(settings.workflow.preset, Some(WorkflowPreset::Ultracode));
    assert!(matches!(
        settings.thinking,
        loopal_provider_api::ThinkingConfig::Auto
    ));
    assert!(settings.workflow.limits.max_nodes <= WorkflowLimits::ABSOLUTE_MAX_NODES);
    assert!(
        settings.workflow.timing.run_deadline_secs
            <= WorkflowTiming::ABSOLUTE_MAX_RUN_DEADLINE_SECS
    );
}

#[test]
fn resolver_keeps_explicit_policy_and_execution_independent() {
    let mut resolver = ConfigResolver::new();
    resolver.add_layer(loopal_config::ConfigLayer {
        source: loopal_config::LayerSource::Project,
        settings: serde_json::json!({"workflow": {"policy": "explicit"}}),
        ..Default::default()
    });
    let resolved = resolver.resolve().unwrap();
    assert_eq!(
        resolved.settings.workflow.policy,
        OrchestrationPolicy::Explicit
    );
    assert!(!resolved.settings.workflow.execution_enabled);

    let mut resolver = ConfigResolver::new();
    resolver.add_layer(loopal_config::ConfigLayer {
        source: loopal_config::LayerSource::Project,
        settings: serde_json::json!({
            "workflow": {
                "policy": "explicit",
                "execution_enabled": true
            }
        }),
        ..Default::default()
    });
    let resolved = resolver.resolve().unwrap();
    assert_eq!(
        resolved.settings.workflow.policy,
        OrchestrationPolicy::Explicit
    );
    assert!(resolved.settings.workflow.execution_enabled);
}

#[test]
fn resolver_rejects_unsafe_zero_values() {
    let mut resolver = ConfigResolver::new();
    resolver.add_layer(loopal_config::ConfigLayer {
        source: loopal_config::LayerSource::Project,
        settings: serde_json::json!({"workflow": {"limits": {"max_nodes": 0}}}),
        ..Default::default()
    });
    let error = resolver.resolve().unwrap_err().to_string();
    assert!(error.contains("workflow"));
    assert!(error.contains("max_nodes"));
}

struct EnvGuard(Vec<(String, Option<String>)>);

impl EnvGuard {
    fn set(values: &[(&str, &str)]) -> Self {
        let previous = values
            .iter()
            .map(|(name, _)| ((*name).into(), std::env::var(name).ok()))
            .collect();
        for (name, value) in values {
            unsafe { std::env::set_var(name, value) };
        }
        Self(previous)
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (name, value) in &self.0 {
            unsafe {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }
}
