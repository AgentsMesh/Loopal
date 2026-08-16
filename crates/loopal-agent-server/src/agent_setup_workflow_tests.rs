use std::sync::Arc;

use loopal_agent::workflow_control::WorkflowControlClient;
use loopal_config::{OrchestrationPolicy, WorkflowSettings};
use loopal_provider_api::{EffortLevel, SharedModelRouter, ThinkingConfig};
use loopal_runtime::workflow_input::WorkflowInputHandler;
use loopal_tool_api::{OneShotChatEffort, OneShotChatError, OneShotChatService};

use super::*;

struct ChatStub;

#[async_trait::async_trait]
impl OneShotChatService for ChatStub {
    async fn one_shot_chat(
        &self,
        _model: &str,
        _system_prompt: &str,
        _user_prompt: &str,
        _max_tokens: u32,
    ) -> Result<String, OneShotChatError> {
        Ok("{}".into())
    }
}

fn connection() -> Arc<Connection<Listening>> {
    let (transport, _peer) = loopal_ipc::duplex_pair();
    let (connection, _incoming) = Connection::new(transport).into_listening();
    connection
}

fn explicit_settings() -> WorkflowSettings {
    WorkflowSettings {
        policy: OrchestrationPolicy::Explicit,
        execution_enabled: true,
        ..WorkflowSettings::default()
    }
}

fn proactive_settings() -> WorkflowSettings {
    WorkflowSettings {
        policy: OrchestrationPolicy::Proactive,
        execution_enabled: true,
        ..WorkflowSettings::default()
    }
}

fn control(settings: &WorkflowSettings) -> Arc<dyn WorkflowControlClient> {
    build_control(
        0,
        settings,
        connection(),
        Arc::new(loopal_runtime::WorkflowLeaseTracker::default()),
    )
    .expect("enabled root workflow control")
}

fn has_control(depth: u32, settings: &WorkflowSettings) -> bool {
    build_control(
        depth,
        settings,
        connection(),
        Arc::new(loopal_runtime::WorkflowLeaseTracker::default()),
    )
    .is_some()
}

fn input_handler(
    depth: u32,
    settings: &WorkflowSettings,
    control: Option<Arc<dyn WorkflowControlClient>>,
) -> Option<Arc<dyn WorkflowInputHandler>> {
    build_input_handler_with_model_router(
        depth,
        settings,
        control,
        Arc::new(ChatStub),
        SharedModelRouter::with_default("planner-model".into()).reader(),
        None,
    )
}

#[tokio::test]
async fn workflow_control_is_root_only_enabled_and_not_off() {
    let enabled = explicit_settings();
    assert!(has_control(0, &enabled));
    assert!(!has_control(1, &enabled));

    let disabled = WorkflowSettings {
        policy: OrchestrationPolicy::Explicit,
        execution_enabled: false,
        ..WorkflowSettings::default()
    };
    assert!(!has_control(0, &disabled));

    let off = WorkflowSettings {
        policy: OrchestrationPolicy::Off,
        execution_enabled: true,
        ..WorkflowSettings::default()
    };
    assert!(!has_control(0, &off));
}

#[tokio::test]
async fn proactive_input_handler_requires_root_enabled_policy_and_control() {
    let proactive = proactive_settings();
    assert!(input_handler(0, &proactive, Some(control(&proactive))).is_some());
    assert!(input_handler(0, &proactive, None).is_none());
    assert!(input_handler(1, &proactive, Some(control(&proactive))).is_none());

    let disabled = WorkflowSettings {
        execution_enabled: false,
        ..proactive.clone()
    };
    assert!(input_handler(0, &disabled, Some(control(&proactive))).is_none());

    let explicit = explicit_settings();
    assert!(input_handler(0, &explicit, Some(control(&explicit))).is_none());
}

#[test]
fn ultracode_max_recommendation_is_the_only_effort_override() {
    assert_eq!(
        planner_options(Some(&ThinkingConfig::Effort {
            level: EffortLevel::Max,
        }))
        .recommended_effort,
        OneShotChatEffort::Max
    );
    assert_eq!(
        planner_options(Some(&ThinkingConfig::Effort {
            level: EffortLevel::High,
        }))
        .recommended_effort,
        OneShotChatEffort::Default
    );
    assert_eq!(
        planner_options(None).recommended_effort,
        OneShotChatEffort::Default
    );
}

#[test]
fn only_one_shot_root_workflow_authority_recovers_pending_deliveries() {
    let enabled = explicit_settings();
    assert!(should_recover_workflows(
        loopal_runtime::LifecycleMode::Ephemeral,
        0,
        &enabled,
    ));
    assert!(should_recover_workflows(
        loopal_runtime::LifecycleMode::WorkflowEphemeral,
        0,
        &enabled,
    ));
    assert!(!should_recover_workflows(
        loopal_runtime::LifecycleMode::Persistent,
        0,
        &enabled,
    ));
    assert!(!should_recover_workflows(
        loopal_runtime::LifecycleMode::Ephemeral,
        1,
        &enabled,
    ));

    let disabled = WorkflowSettings {
        execution_enabled: false,
        ..enabled.clone()
    };
    assert!(!should_recover_workflows(
        loopal_runtime::LifecycleMode::Ephemeral,
        0,
        &disabled,
    ));
    let off = WorkflowSettings {
        policy: OrchestrationPolicy::Off,
        ..enabled
    };
    assert!(!should_recover_workflows(
        loopal_runtime::LifecycleMode::Ephemeral,
        0,
        &off,
    ));
}
