use loopal_config::{OrchestrationPolicy, Settings, WorkflowPreset, WorkflowSettings};

use super::production_workflow_execution_enabled;

#[test]
fn defaults_and_ultracode_preset_keep_production_execution_latched() {
    assert!(!production_workflow_execution_enabled(
        &WorkflowSettings::default()
    ));

    let mut settings = Settings::default();
    settings.workflow.preset = Some(WorkflowPreset::Ultracode);
    let workflow = settings.resolve_workflow_preset().settings.workflow;

    assert_eq!(workflow.policy, OrchestrationPolicy::Proactive);
    assert!(!workflow.execution_enabled);
    assert!(!production_workflow_execution_enabled(&workflow));
}

#[test]
fn explicit_policy_requires_the_independent_execution_opt_in() {
    let mut workflow = WorkflowSettings {
        policy: OrchestrationPolicy::Explicit,
        ..WorkflowSettings::default()
    };
    assert!(!production_workflow_execution_enabled(&workflow));

    workflow.execution_enabled = true;
    assert!(production_workflow_execution_enabled(&workflow));
}

#[test]
fn off_policy_closes_execution_even_when_the_flag_is_set() {
    let workflow = WorkflowSettings {
        execution_enabled: true,
        ..WorkflowSettings::default()
    };

    assert!(!production_workflow_execution_enabled(&workflow));
}
