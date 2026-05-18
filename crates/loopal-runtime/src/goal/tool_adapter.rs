use std::sync::Arc;

use async_trait::async_trait;
use loopal_protocol::{GoalTransitionReason, ThreadGoal, ThreadGoalStatus};
use loopal_tool_api::{GoalSession, GoalSessionError};

use super::session::GoalRuntimeSession;

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

    async fn create(&self, objective: String) -> Result<ThreadGoal, GoalSessionError> {
        self.inner.create(objective).await
    }

    async fn complete_by_model(&self) -> Result<ThreadGoal, GoalSessionError> {
        self.inner
            .transition(
                ThreadGoalStatus::Complete,
                GoalTransitionReason::ModelCompleted,
            )
            .await
    }

    async fn reopen_by_model(&self) -> Result<ThreadGoal, GoalSessionError> {
        self.inner
            .transition(
                ThreadGoalStatus::Active,
                GoalTransitionReason::ModelReopened,
            )
            .await
    }
}
