use async_trait::async_trait;
use loopal_protocol::ThreadGoal;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GoalSessionError {
    #[error("a goal already exists for this session")]
    AlreadyExists,
    #[error("no goal exists for this session")]
    NotFound,
    #[error("model can only mark goals complete; pause/resume are user-only")]
    ModelStatusForbidden,
    #[error("goal token budget must be positive when provided")]
    InvalidBudget,
    #[error("objective must be non-empty and at most {max} characters; got {got}")]
    ObjectiveTooLong { max: usize, got: usize },
    #[error("goal storage error: {0}")]
    Storage(String),
}

#[async_trait]
pub trait GoalSession: Send + Sync {
    async fn snapshot(&self) -> Result<Option<ThreadGoal>, GoalSessionError>;

    async fn create(
        &self,
        objective: String,
        token_budget: Option<u64>,
    ) -> Result<ThreadGoal, GoalSessionError>;

    async fn complete_by_model(&self) -> Result<ThreadGoal, GoalSessionError>;
}
