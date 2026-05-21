use async_trait::async_trait;
use loopal_protocol::ThreadGoal;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GoalSessionError {
    #[error("a goal already exists for this session")]
    AlreadyExists,
    #[error("no goal exists for this session")]
    NotFound,
    #[error("model attempted a forbidden goal status transition")]
    ModelStatusForbidden,
    #[error("objective must be non-empty and at most {max} characters; got {got}")]
    ObjectiveTooLong { max: usize, got: usize },
    #[error("goal storage error: {0}")]
    Storage(String),
}

#[async_trait]
pub trait GoalSession: Send + Sync {
    async fn snapshot(&self) -> Result<Option<ThreadGoal>, GoalSessionError>;

    async fn create(&self, objective: String) -> Result<ThreadGoal, GoalSessionError>;

    async fn complete_by_model(&self) -> Result<ThreadGoal, GoalSessionError>;

    async fn reopen_by_model(&self) -> Result<ThreadGoal, GoalSessionError>;

    async fn mark_infeasible_by_model(&self) -> Result<ThreadGoal, GoalSessionError>;
}
