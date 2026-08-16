use std::sync::Arc;

use async_trait::async_trait;
use loopal_config::{WorkflowPlannerProfile, WorkflowSettings};
use loopal_protocol::{
    WorkflowExecution, WorkflowPlanDecision, cap_and_validate_workflow,
    is_deterministically_simple_goal, parse_workflow_plan, workflow_plan_schema,
};
use loopal_provider_api::ModelRouterReader;
use loopal_tool_api::{OneShotChatEffort, OneShotChatOptions, OneShotChatService};

#[path = "workflow_planner_model.rs"]
mod workflow_planner_model;
#[path = "workflow_planner_prompt.rs"]
mod workflow_planner_prompt;

use workflow_planner_model::PlannerModel;
use workflow_planner_prompt::{
    PLANNER_MAX_TOKENS, PLANNER_SYSTEM_PROMPT, cap_reason, ceilings, direct, planner_user_prompt,
};

pub use workflow_planner_model::WorkflowPlannerOptions;

#[derive(Clone)]
pub struct ProactiveWorkflowPlanner {
    settings: WorkflowSettings,
    chat: Arc<dyn OneShotChatService>,
    model: PlannerModel,
    options: WorkflowPlannerOptions,
}

impl ProactiveWorkflowPlanner {
    pub fn new(
        settings: WorkflowSettings,
        chat: Arc<dyn OneShotChatService>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            settings,
            chat,
            model: PlannerModel::Fixed(model.into()),
            options: WorkflowPlannerOptions::default(),
        }
    }

    pub fn new_with_options(
        settings: WorkflowSettings,
        chat: Arc<dyn OneShotChatService>,
        model: impl Into<String>,
        options: WorkflowPlannerOptions,
    ) -> Self {
        Self {
            settings,
            chat,
            model: PlannerModel::Fixed(model.into()),
            options,
        }
    }

    /// Construct a planner backed by the session's shared model router.
    /// Runtime model switches are then visible to every subsequent plan.
    pub fn new_with_model_router(
        settings: WorkflowSettings,
        chat: Arc<dyn OneShotChatService>,
        model_router: ModelRouterReader,
    ) -> Self {
        Self {
            settings,
            chat,
            model: PlannerModel::Shared(model_router),
            options: WorkflowPlannerOptions::default(),
        }
    }

    pub fn new_with_model_router_and_options(
        settings: WorkflowSettings,
        chat: Arc<dyn OneShotChatService>,
        model_router: ModelRouterReader,
        options: WorkflowPlannerOptions,
    ) -> Self {
        Self {
            settings,
            chat,
            model: PlannerModel::Shared(model_router),
            options,
        }
    }

    pub fn settings(&self) -> &WorkflowSettings {
        &self.settings
    }

    pub async fn plan(&self, goal: &str, recent_context: &str) -> WorkflowPlanDecision {
        if !self.settings.execution_enabled
            || self.settings.policy != loopal_config::OrchestrationPolicy::Proactive
        {
            return direct("proactive orchestration is disabled");
        }
        if is_deterministically_simple_goal(goal) {
            return direct("small task uses the direct agent loop");
        }
        let ceilings = ceilings(&self.settings);
        let prompt = planner_user_prompt(goal, recent_context, ceilings);
        let model = self.model.current();
        let raw = match if self.settings.planner_profile == WorkflowPlannerProfile::Ultracode {
            // A manually constructed Ultracode profile predates the separate
            // preset recommendation field, so retain its historical Max
            // default when setup did not provide one.
            let effort = match self.options.recommended_effort {
                OneShotChatEffort::Default => OneShotChatEffort::Max,
                effort => effort,
            };
            self.chat
                .one_shot_chat_with_options(
                    &model,
                    PLANNER_SYSTEM_PROMPT,
                    &prompt,
                    PLANNER_MAX_TOKENS,
                    OneShotChatOptions::new(effort),
                )
                .await
        } else {
            self.chat
                .one_shot_chat(&model, PLANNER_SYSTEM_PROMPT, &prompt, PLANNER_MAX_TOKENS)
                .await
        } {
            Ok(raw) => raw,
            Err(error) => return direct(format!("planner unavailable: {error}")),
        };
        let decision = match parse_workflow_plan(&raw) {
            Ok(decision) => decision,
            Err(error) => return direct(format!("planner output rejected: {error}")),
        };
        match decision.execution {
            WorkflowExecution::Direct { reason } => WorkflowPlanDecision {
                version: decision.version,
                execution: WorkflowExecution::Direct {
                    reason: reason.map(|value| cap_reason(&value)),
                },
            },
            WorkflowExecution::Workflow { spec } => {
                match cap_and_validate_workflow(spec, ceilings) {
                    Ok(spec) => WorkflowPlanDecision {
                        version: decision.version,
                        execution: WorkflowExecution::Workflow { spec },
                    },
                    Err(error) => direct(format!("workflow candidate rejected: {error}")),
                }
            }
        }
    }

    /// Exposes the exact schema used by integrations that support structured
    /// output. Parsing remains mandatory because providers may ignore schemas.
    pub fn response_schema() -> serde_json::Value {
        workflow_plan_schema()
    }
}

#[async_trait]
pub trait WorkflowPlanner: Send + Sync {
    async fn plan(&self, goal: &str, recent_context: &str) -> WorkflowPlanDecision;
}

#[async_trait]
impl WorkflowPlanner for ProactiveWorkflowPlanner {
    async fn plan(&self, goal: &str, recent_context: &str) -> WorkflowPlanDecision {
        Self::plan(self, goal, recent_context).await
    }
}

#[cfg(test)]
#[path = "workflow_planner_tests.rs"]
mod tests;
