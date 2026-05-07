use std::sync::{Arc, Mutex as StdMutex};

use loopal_protocol::{AgentEventPayload, GoalTransitionReason, ThreadGoal, ThreadGoalStatus};
use loopal_storage::GoalStore;
use loopal_tool_api::GoalSessionError;
use tokio::sync::Mutex;
use tracing::warn;

use crate::frontend::traits::EventEmitter;

/// Outcome of [`GoalRuntimeSession::add_usage`]. Callers use this to react
/// in-process (e.g. inject a budget-limit warning into the current turn)
/// rather than waiting for the broadcast `ThreadGoalUpdated` event to round
/// trip back through Hub → ViewState.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageOutcome {
    NoOp,
    Updated,
    BudgetExhausted,
}

/// Maximum objective length in characters. Long objectives bloat goal.json
/// and recur in every continuation prompt.
pub const MAX_OBJECTIVE_CHARS: usize = 4096;

/// Authoritative per-session goal owner. All mutations funnel through here so
/// the state machine stays single-writer.
///
/// Mutating methods commit to disk before emitting `ThreadGoalUpdated`; emit
/// failures are logged at WARN, never rolled back — UIs reconcile via the
/// next `agent/state_snapshot`.
pub struct GoalRuntimeSession {
    session_id: StdMutex<String>,
    pub(super) store: Arc<GoalStore>,
    pub(super) emitter: Box<dyn EventEmitter>,
    pub(super) write_lock: Mutex<()>,
}

impl GoalRuntimeSession {
    pub fn new(session_id: String, store: Arc<GoalStore>, emitter: Box<dyn EventEmitter>) -> Self {
        debug_assert!(
            !session_id.trim().is_empty(),
            "GoalRuntimeSession: session_id must be non-empty"
        );
        Self {
            session_id: StdMutex::new(session_id),
            store,
            emitter,
            write_lock: Mutex::new(()),
        }
    }

    pub(super) fn current_session_id(&self) -> String {
        self.session_id.lock().unwrap().clone()
    }

    pub async fn set_session_id(&self, new: String) -> Result<(), GoalSessionError> {
        if new.trim().is_empty() {
            return Err(GoalSessionError::Storage(
                "session_id must be non-empty".into(),
            ));
        }
        let _guard = self.write_lock.lock().await;
        *self.session_id.lock().unwrap() = new;
        Ok(())
    }

    pub async fn snapshot(&self) -> Result<Option<ThreadGoal>, GoalSessionError> {
        self.store
            .load(&self.current_session_id())
            .map_err(|e| GoalSessionError::Storage(e.to_string()))
    }

    pub async fn create(
        &self,
        objective: String,
        token_budget: Option<u64>,
    ) -> Result<ThreadGoal, GoalSessionError> {
        let len = objective.chars().count();
        if !(1..=MAX_OBJECTIVE_CHARS).contains(&len) {
            return Err(GoalSessionError::ObjectiveTooLong {
                max: MAX_OBJECTIVE_CHARS,
                got: len,
            });
        }
        if matches!(token_budget, Some(0)) {
            return Err(GoalSessionError::InvalidBudget);
        }
        let goal = {
            let _guard = self.write_lock.lock().await;
            let id = self.current_session_id();
            if let Some(existing) = self
                .store
                .load(&id)
                .map_err(|e| GoalSessionError::Storage(e.to_string()))?
                && existing.status != ThreadGoalStatus::Complete
            {
                return Err(GoalSessionError::AlreadyExists);
            }
            let mut goal = ThreadGoal::new(id, objective);
            goal.token_budget = token_budget;
            self.store
                .save(&goal)
                .map_err(|e| GoalSessionError::Storage(e.to_string()))?;
            goal
        };
        self.emit_updated(Some(goal.clone()), GoalTransitionReason::UserCreated)
            .await;
        Ok(goal)
    }

    pub async fn transition(
        &self,
        new_status: ThreadGoalStatus,
        reason: GoalTransitionReason,
    ) -> Result<ThreadGoal, GoalSessionError> {
        let goal = {
            let _guard = self.write_lock.lock().await;
            let id = self.current_session_id();
            let mut goal = self
                .store
                .load(&id)
                .map_err(|e| GoalSessionError::Storage(e.to_string()))?
                .ok_or(GoalSessionError::NotFound)?;
            if !goal.status.can_transition_to(new_status, reason) {
                return Err(GoalSessionError::ModelStatusForbidden);
            }
            goal.status = new_status;
            goal.updated_at = chrono::Utc::now();
            self.store
                .save(&goal)
                .map_err(|e| GoalSessionError::Storage(e.to_string()))?;
            goal
        };
        self.emit_updated(Some(goal.clone()), reason).await;
        Ok(goal)
    }

    pub async fn clear(&self) -> Result<(), GoalSessionError> {
        let existed = {
            let _guard = self.write_lock.lock().await;
            let id = self.current_session_id();
            let existed = self
                .store
                .load(&id)
                .map_err(|e| GoalSessionError::Storage(e.to_string()))?
                .is_some();
            self.store
                .clear(&id)
                .map_err(|e| GoalSessionError::Storage(e.to_string()))?;
            existed
        };
        if existed {
            self.emit_updated(None, GoalTransitionReason::UserCleared)
                .await;
        }
        Ok(())
    }

    pub(super) async fn emit_updated(
        &self,
        goal: Option<ThreadGoal>,
        reason: GoalTransitionReason,
    ) {
        if let Err(err) = self
            .emitter
            .emit(AgentEventPayload::ThreadGoalUpdated { goal, reason })
            .await
        {
            warn!(error = %err, "failed to emit ThreadGoalUpdated");
        }
    }
}
