use std::sync::Arc;

use async_trait::async_trait;
use loopal_protocol::{GoalTransitionReason, ThreadGoal, ThreadGoalStatus};
use loopal_tool_api::{GoalSession, GoalSessionError};

use super::session::GoalRuntimeSession;

/// Narrow view of [`GoalRuntimeSession`] exposed to LLM-facing tools.
///
/// This adapter intentionally only forwards `snapshot`, `create`, and the
/// model-only `complete_by_model` path. User/system transitions (pause,
/// resume, budget-limited) are handled outside the tool surface.
pub struct GoalSessionToolAdapter {
    inner: Arc<GoalRuntimeSession>,
}

impl GoalSessionToolAdapter {
    pub fn new(inner: Arc<GoalRuntimeSession>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl GoalSession for GoalSessionToolAdapter {
    async fn snapshot(&self) -> Result<Option<ThreadGoal>, GoalSessionError> {
        self.inner.snapshot().await
    }

    async fn create(
        &self,
        objective: String,
        token_budget: Option<u64>,
    ) -> Result<ThreadGoal, GoalSessionError> {
        self.inner.create(objective, token_budget).await
    }

    async fn complete_by_model(&self) -> Result<ThreadGoal, GoalSessionError> {
        self.inner
            .transition(
                ThreadGoalStatus::Complete,
                GoalTransitionReason::ModelCompleted,
            )
            .await
    }
}
