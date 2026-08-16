use std::path::PathBuf;

use loopal_config::settings::SecretsSettings;
use loopal_config::{
    FileSystemPolicy, NetworkPolicy, OrchestrationPolicy, SandboxConfig, SandboxPolicy, Settings,
    WorkflowPlannerProfile, WorkflowPreset,
};
use loopal_decision_api::DecisionMode;
use loopal_provider_api::{EffortLevel, ThinkingConfig};
use loopal_tool_api::PermissionMode;

#[test]
fn ultracode_resolution_is_pure_latched_and_recommends_max_effort() {
    let mut settings = security_sensitive_settings();
    settings.workflow.preset = Some(WorkflowPreset::Ultracode);
    let original = serde_json::to_value(&settings).unwrap();

    let resolution = settings.resolve_workflow_preset();

    assert_eq!(serde_json::to_value(&settings).unwrap(), original);
    assert_eq!(
        resolution.settings.workflow.policy,
        OrchestrationPolicy::Proactive
    );
    assert!(!resolution.settings.workflow.execution_enabled);
    assert_eq!(
        resolution.settings.workflow.planner_profile,
        WorkflowPlannerProfile::Ultracode
    );
    assert!(matches!(
        resolution.recommended_thinking,
        Some(ThinkingConfig::Effort {
            level: EffortLevel::Max
        })
    ));
    assert!(matches!(resolution.settings.thinking, ThinkingConfig::Auto));
    assert!(resolution.settings.workflow.validate().is_ok());
}

#[test]
fn ultracode_preserves_explicit_execution_opt_in() {
    let mut settings = Settings::default();
    settings.workflow.execution_enabled = true;
    settings.workflow.preset = Some(WorkflowPreset::Ultracode);

    let resolved = settings.resolve_workflow_preset().settings;

    assert!(resolved.workflow.execution_enabled);
    assert_eq!(resolved.workflow.policy, OrchestrationPolicy::Proactive);
}

#[test]
fn ultracode_preserves_thinking_until_capability_aware_resolution() {
    for thinking in [
        ThinkingConfig::Auto,
        ThinkingConfig::Disabled,
        ThinkingConfig::Effort {
            level: EffortLevel::Low,
        },
        ThinkingConfig::Budget { tokens: 4096 },
    ] {
        let mut settings = Settings::default();
        settings.workflow.preset = Some(WorkflowPreset::Ultracode);
        settings.thinking = thinking;
        let expected = serde_json::to_value(&settings.thinking).unwrap();
        let resolution = settings.resolve_workflow_preset();
        assert_eq!(
            serde_json::to_value(&resolution.settings.thinking).unwrap(),
            expected
        );
        assert!(matches!(
            resolution.recommended_thinking,
            Some(ThinkingConfig::Effort {
                level: EffortLevel::Max
            })
        ));
    }
}

#[test]
fn ultracode_preserves_permission_decision_sandbox_and_secrets() {
    let mut settings = security_sensitive_settings();
    settings.workflow.preset = Some(WorkflowPreset::Ultracode);
    let expected_permission = settings.permission_mode;
    let expected_decision = settings.decision_mode;
    let expected_sandbox = serde_json::to_value(&settings.sandbox).unwrap();
    let expected_secrets = serde_json::to_value(&settings.secrets).unwrap();

    let resolved = settings.resolve_workflow_preset().settings;

    assert_eq!(resolved.permission_mode, expected_permission);
    assert_eq!(resolved.decision_mode, expected_decision);
    assert_eq!(
        serde_json::to_value(&resolved.sandbox).unwrap(),
        expected_sandbox
    );
    assert_eq!(
        serde_json::to_value(&resolved.secrets).unwrap(),
        expected_secrets
    );
}

#[test]
fn no_preset_leaves_workflow_unchanged_and_has_no_effort_recommendation() {
    let settings = security_sensitive_settings();
    let expected = serde_json::to_value(&settings).unwrap();
    let resolution = settings.resolve_workflow_preset();
    assert_eq!(
        serde_json::to_value(&resolution.settings).unwrap(),
        expected
    );
    assert!(resolution.recommended_thinking.is_none());
}

fn security_sensitive_settings() -> Settings {
    Settings {
        permission_mode: PermissionMode::AskAnyWrite,
        decision_mode: DecisionMode::Classifier,
        sandbox: SandboxConfig {
            policy: SandboxPolicy::ReadOnly,
            filesystem: FileSystemPolicy {
                allow_write: vec!["/safe".into()],
                deny_write: vec!["**/.git/**".into()],
                deny_read: vec!["**/.env".into()],
            },
            network: NetworkPolicy {
                allowed_domains: vec!["example.com".into()],
                denied_domains: vec!["blocked.example".into()],
            },
        },
        secrets: SecretsSettings {
            vaults_dir: Some(PathBuf::from("/vaults")),
            default_vault: Some("restricted".into()),
        },
        ..Settings::default()
    }
}
