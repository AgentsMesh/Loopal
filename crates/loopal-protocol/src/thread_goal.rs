use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadGoalStatus {
    Active,
    Paused,
    BudgetLimited,
    Complete,
}

impl ThreadGoalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::BudgetLimited => "budget_limited",
            Self::Complete => "complete",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Complete)
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_budget: Option<u64>,
    pub tokens_used: u64,
    pub time_used_ms: u64,
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
            token_budget: None,
            tokens_used: 0,
            time_used_ms: 0,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_token_budget(mut self, budget: u64) -> Self {
        self.token_budget = Some(budget);
        self
    }

    pub fn remaining_tokens(&self) -> Option<u64> {
        self.token_budget
            .map(|b| b.saturating_sub(self.tokens_used))
    }

    pub fn budget_exhausted(&self) -> bool {
        match self.token_budget {
            Some(b) => self.tokens_used >= b,
            None => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalTransitionReason {
    UserCreated,
    ModelCompleted,
    UserCompleted,
    BudgetExhausted,
    UserPaused,
    UserResumed,
    UserExtendedBudget,
    UserCleared,
    BarrenContinuation,
    UsageUpdated,
}

impl ThreadGoalStatus {
    pub fn can_transition_to(self, next: ThreadGoalStatus, reason: GoalTransitionReason) -> bool {
        use GoalTransitionReason as R;
        use ThreadGoalStatus as S;
        matches!(
            (self, next, reason),
            (S::Active, S::Complete, R::ModelCompleted | R::UserCompleted)
                | (
                    S::Active,
                    S::BudgetLimited,
                    R::BudgetExhausted | R::BarrenContinuation,
                )
                | (S::Active, S::Paused, R::UserPaused)
                | (S::Paused, S::Active, R::UserResumed)
                | (S::Paused, S::Complete, R::UserCompleted)
                | (S::BudgetLimited, S::Active, R::UserExtendedBudget)
                | (
                    S::BudgetLimited,
                    S::Complete,
                    R::UserCompleted | R::ModelCompleted,
                )
        )
    }
}
