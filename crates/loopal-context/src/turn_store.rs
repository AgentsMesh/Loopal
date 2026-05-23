use std::time::SystemTime;

use loopal_turn::{Turn, TurnId, TurnOutcome, TurnStep, TurnTrigger};

use crate::budget::ContextBudget;

#[derive(Debug, thiserror::Error)]
pub enum TurnStoreError {
    #[error("no turn in progress")]
    NoCurrentTurn,
    #[error("current turn is not in progress (outcome already set)")]
    CurrentTurnFinished,
    #[error("turn not found: {0}")]
    TurnNotFound(String),
}

pub type TurnStoreResult<T> = Result<T, TurnStoreError>;

/// In-memory store of [`Turn`]s for a single session.
///
/// Replaces the message-shaped [`crate::ContextStore`]. Domain entity is `Turn`;
/// LLM wire-format messages are derived only at provider boundary.
pub struct TurnStore {
    turns: Vec<Turn>,
    current_turn_id: Option<TurnId>,
    budget: ContextBudget,
    last_actual_input_tokens: Option<u32>,
    last_assistant_activity_at: Option<SystemTime>,
}

impl TurnStore {
    pub fn new(budget: ContextBudget) -> Self {
        Self {
            turns: Vec::new(),
            current_turn_id: None,
            budget,
            last_actual_input_tokens: None,
            last_assistant_activity_at: None,
        }
    }

    pub fn from_turns(turns: Vec<Turn>, budget: ContextBudget) -> Self {
        // reason: 从存储 resume 时，最后一个 InProgress turn (如有) 作为 current。
        let current_turn_id = turns
            .iter()
            .rev()
            .find(|t| matches!(t.outcome, TurnOutcome::InProgress))
            .map(|t| t.id.clone());
        Self {
            turns,
            current_turn_id,
            budget,
            last_actual_input_tokens: None,
            last_assistant_activity_at: None,
        }
    }

    pub fn start_turn(&mut self, trigger: TurnTrigger) -> TurnId {
        let turn = Turn::new(trigger);
        let id = turn.id.clone();
        self.turns.push(turn);
        self.current_turn_id = Some(id.clone());
        id
    }

    pub fn append_step(&mut self, step: TurnStep) -> TurnStoreResult<u32> {
        let id = self
            .current_turn_id
            .as_ref()
            .ok_or(TurnStoreError::NoCurrentTurn)?
            .clone();
        let turn = self
            .turns
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| TurnStoreError::TurnNotFound(id.as_str().to_string()))?;
        if !matches!(turn.outcome, TurnOutcome::InProgress) {
            return Err(TurnStoreError::CurrentTurnFinished);
        }
        let idx = turn.body.steps.len() as u32;
        turn.body.steps.push(step);
        Ok(idx)
    }

    pub fn end_current_turn(&mut self, outcome: TurnOutcome) -> TurnStoreResult<()> {
        let id = self
            .current_turn_id
            .take()
            .ok_or(TurnStoreError::NoCurrentTurn)?;
        let turn = self
            .turns
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| TurnStoreError::TurnNotFound(id.as_str().to_string()))?;
        turn.outcome = outcome;
        Ok(())
    }

    pub fn turns(&self) -> &[Turn] {
        &self.turns
    }

    pub fn turns_mut(&mut self) -> &mut Vec<Turn> {
        &mut self.turns
    }

    pub fn current_turn(&self) -> Option<&Turn> {
        let id = self.current_turn_id.as_ref()?;
        self.turns.iter().find(|t| &t.id == id)
    }

    pub fn current_turn_id(&self) -> Option<&TurnId> {
        self.current_turn_id.as_ref()
    }

    pub fn len(&self) -> usize {
        self.turns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }

    pub fn budget(&self) -> &ContextBudget {
        &self.budget
    }

    pub fn update_budget(&mut self, budget: ContextBudget) {
        self.budget = budget;
    }

    pub fn clear(&mut self) {
        self.turns.clear();
        self.current_turn_id = None;
    }

    pub fn record_actual_input_tokens(&mut self, tokens: u32) {
        self.last_actual_input_tokens = Some(tokens);
    }

    pub fn last_actual_input_tokens(&self) -> Option<u32> {
        self.last_actual_input_tokens
    }

    pub fn record_assistant_activity(&mut self, at: SystemTime) {
        self.last_assistant_activity_at = Some(at);
    }

    pub fn last_assistant_activity_at(&self) -> Option<SystemTime> {
        self.last_assistant_activity_at
    }
}
