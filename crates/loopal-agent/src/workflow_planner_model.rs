use loopal_provider_api::{ModelRouterReader, TaskType};
use loopal_tool_api::OneShotChatEffort;

/// Policy inputs owned by setup and carried into the planner. The profile is
/// still read from `WorkflowSettings`; this value carries the independently
/// resolved preset recommendation without coupling the planner to config's
/// `ThinkingConfig` type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkflowPlannerOptions {
    pub recommended_effort: OneShotChatEffort,
}

/// Model source used by the planner. Production setup supplies the shared
/// read-only router so `/model` switches are observed without giving the
/// planner write access. The fixed variant keeps the original constructor
/// useful for tests and external callers.
#[derive(Clone)]
pub(crate) enum PlannerModel {
    Fixed(String),
    Shared(ModelRouterReader),
}

impl PlannerModel {
    pub(crate) fn current(&self) -> String {
        match self {
            Self::Fixed(model) => model.clone(),
            Self::Shared(router) => router.read().resolve(TaskType::Default).to_string(),
        }
    }
}
