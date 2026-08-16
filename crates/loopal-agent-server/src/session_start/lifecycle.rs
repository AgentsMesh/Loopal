use loopal_config::{OrchestrationPolicy, WorkflowSettings};
use loopal_runtime::LifecycleMode;

/// Apply the workflow-aware default only when the caller did not choose a
/// lifecycle and this is a root prompt that can actually start workflows.
pub(crate) fn select_lifecycle(
    current: LifecycleMode,
    lifecycle_explicit: bool,
    prompt: Option<&str>,
    depth: u32,
    workflow: &WorkflowSettings,
) -> LifecycleMode {
    if !lifecycle_explicit
        && prompt.is_some_and(|value| !value.is_empty())
        && depth == 0
        && workflow.execution_enabled
        && workflow.policy != OrchestrationPolicy::Off
    {
        LifecycleMode::WorkflowEphemeral
    } else {
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled_workflow() -> WorkflowSettings {
        WorkflowSettings {
            execution_enabled: true,
            policy: OrchestrationPolicy::Proactive,
            ..WorkflowSettings::default()
        }
    }

    #[test]
    fn upgrades_an_implicit_root_prompt_when_workflows_are_enabled() {
        assert_eq!(
            select_lifecycle(
                LifecycleMode::Ephemeral,
                false,
                Some("delegate this"),
                0,
                &enabled_workflow(),
            ),
            LifecycleMode::WorkflowEphemeral
        );
    }

    #[test]
    fn explicit_ephemeral_is_never_upgraded() {
        assert_eq!(
            select_lifecycle(
                LifecycleMode::Ephemeral,
                true,
                Some("delegate this"),
                0,
                &enabled_workflow(),
            ),
            LifecycleMode::Ephemeral
        );
    }

    #[test]
    fn workflow_off_keeps_the_ordinary_ephemeral_default() {
        let workflow = WorkflowSettings {
            execution_enabled: true,
            policy: OrchestrationPolicy::Off,
            ..WorkflowSettings::default()
        };
        assert_eq!(
            select_lifecycle(
                LifecycleMode::Ephemeral,
                false,
                Some("delegate this"),
                0,
                &workflow,
            ),
            LifecycleMode::Ephemeral
        );
    }

    #[test]
    fn child_and_empty_prompts_are_never_upgraded() {
        let workflow = enabled_workflow();
        assert_eq!(
            select_lifecycle(
                LifecycleMode::Ephemeral,
                false,
                Some("delegate this"),
                1,
                &workflow,
            ),
            LifecycleMode::Ephemeral
        );
        assert_eq!(
            select_lifecycle(LifecycleMode::Ephemeral, false, Some(""), 0, &workflow),
            LifecycleMode::Ephemeral
        );
    }
}
