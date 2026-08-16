use std::sync::Arc;

use loopal_agent::workflow_control::WorkflowControlClient;
use loopal_ipc::connection::{Connection, Listening};
use loopal_provider_api::{EffortLevel, ModelRouterReader, ThinkingConfig, ThinkingConfigReader};
use loopal_runtime::workflow_input::WorkflowInputHandler;
use loopal_tool_api::{OneShotChatService, OutstandingTasksDigest};

fn has_workflow_authority(depth: u32, settings: &loopal_config::WorkflowSettings) -> bool {
    depth == 0
        && settings.execution_enabled
        && settings.policy != loopal_config::OrchestrationPolicy::Off
}

pub(crate) fn should_recover_workflows(
    lifecycle: loopal_runtime::LifecycleMode,
    depth: u32,
    settings: &loopal_config::WorkflowSettings,
) -> bool {
    lifecycle.is_one_shot() && has_workflow_authority(depth, settings)
}

pub(crate) fn build_control(
    depth: u32,
    settings: &loopal_config::WorkflowSettings,
    hub_connection: Arc<Connection<Listening>>,
    tracker: Arc<loopal_runtime::WorkflowLeaseTracker>,
) -> Option<Arc<dyn WorkflowControlClient>> {
    if has_workflow_authority(depth, settings) {
        let inner = Arc::new(
            loopal_agent::workflow_control::ConnectionWorkflowControlClient::new(hub_connection),
        ) as Arc<dyn WorkflowControlClient>;
        Some(Arc::new(
            crate::workflow_control_tracking::TrackingWorkflowControlClient::new(inner, tracker),
        ) as Arc<dyn WorkflowControlClient>)
    } else {
        None
    }
}

/// Setup variant that keeps the planner bound to the session's shared model
/// router. The runtime mutates that router for `/model`; using its reader here
/// prevents proactive planning from retaining a stale startup model.
pub(crate) fn build_input_handler_with_model_router(
    depth: u32,
    settings: &loopal_config::WorkflowSettings,
    control: Option<Arc<dyn WorkflowControlClient>>,
    one_shot_chat: Arc<dyn OneShotChatService>,
    model_router: ModelRouterReader,
    recommendation: Option<ThinkingConfig>,
) -> Option<Arc<dyn WorkflowInputHandler>> {
    build_input_handler_impl(
        depth,
        settings,
        control,
        loopal_agent::ProactiveWorkflowPlanner::new_with_model_router_and_options(
            settings.clone(),
            one_shot_chat,
            model_router,
            planner_options(recommendation.as_ref()),
        ),
    )
}

fn planner_options(
    recommendation: Option<&ThinkingConfig>,
) -> loopal_agent::WorkflowPlannerOptions {
    let recommended_effort = match recommendation {
        Some(ThinkingConfig::Effort {
            level: EffortLevel::Max,
        }) => loopal_tool_api::OneShotChatEffort::Max,
        _ => loopal_tool_api::OneShotChatEffort::Default,
    };
    loopal_agent::WorkflowPlannerOptions { recommended_effort }
}

fn build_input_handler_impl(
    depth: u32,
    settings: &loopal_config::WorkflowSettings,
    control: Option<Arc<dyn WorkflowControlClient>>,
    planner: loopal_agent::ProactiveWorkflowPlanner,
) -> Option<Arc<dyn WorkflowInputHandler>> {
    if depth == 0
        && settings.execution_enabled
        && settings.policy == loopal_config::OrchestrationPolicy::Proactive
    {
        control.map(|control| {
            Arc::new(crate::workflow_input::ProactiveWorkflowInputHandler::new(
                planner, control,
            )) as Arc<dyn WorkflowInputHandler>
        })
    } else {
        None
    }
}

pub(crate) struct AgentServices {
    pub shared: Arc<dyn std::any::Any + Send + Sync>,
    pub one_shot_chat: Arc<dyn OneShotChatService>,
    pub fetch_refiner: Arc<dyn loopal_tool_api::FetchRefinerPolicy>,
    pub outstanding_tasks: Arc<dyn OutstandingTasksDigest>,
}

pub(crate) fn services(
    agent: &Arc<loopal_agent::shared::AgentShared>,
    thinking: ThinkingConfigReader,
) -> AgentServices {
    AgentServices {
        shared: Arc::new(agent.clone()),
        one_shot_chat: Arc::new(loopal_agent::LiveOneShotChatService::new(
            agent.clone(),
            thinking,
        )),
        fetch_refiner: agent.clone(),
        outstanding_tasks: agent.clone(),
    }
}

#[cfg(test)]
#[path = "agent_setup_workflow_tests.rs"]
mod tests;
