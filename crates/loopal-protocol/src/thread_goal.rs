use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadGoalStatus {
    Active,
    Paused,
    Complete,
    Infeasible,
}

impl ThreadGoalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Complete => "complete",
            Self::Infeasible => "infeasible",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Complete | Self::Infeasible)
    }

    pub fn participates_in_continuation(self) -> bool {
        matches!(self, Self::Active)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadGoal {
    pub session_id: String,
    pub goal_id: String,
    pub objective: String,
    pub status: ThreadGoalStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ThreadGoal {
    pub fn new(session_id: impl Into<String>, objective: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            session_id: session_id.into(),
            goal_id: uuid::Uuid::new_v4().to_string(),
            objective: objective.into(),
            status: ThreadGoalStatus::Active,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalTransitionReason {
    UserCreated,
    ModelCompleted,
    UserCompleted,
    ModelReopened,
    UserReopened,
    UserPaused,
    UserResumed,
    UserCleared,
    ModelInfeasible,
    UserInfeasible,
}

impl ThreadGoalStatus {
    pub fn can_transition_to(self, next: ThreadGoalStatus, reason: GoalTransitionReason) -> bool {
        use GoalTransitionReason as R;
        use ThreadGoalStatus as S;
        matches!(
            (self, next, reason),
            (S::Active, S::Complete, R::ModelCompleted | R::UserCompleted)
                | (S::Active, S::Paused, R::UserPaused)
                | (
                    S::Active,
                    S::Infeasible,
                    R::ModelInfeasible | R::UserInfeasible
                )
                | (S::Paused, S::Active, R::UserResumed)
                | (S::Paused, S::Complete, R::UserCompleted)
                | (S::Paused, S::Infeasible, R::UserInfeasible)
                | (S::Complete, S::Active, R::ModelReopened | R::UserReopened)
                | (S::Infeasible, S::Active, R::ModelReopened | R::UserReopened)
        )
    }
}
