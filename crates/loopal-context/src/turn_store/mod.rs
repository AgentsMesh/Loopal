mod query;
mod reopen;
mod rollback;

use loopal_turn::{ToolExecState, Turn, TurnId, TurnOutcome, TurnStep, TurnTrigger};

#[derive(Debug, thiserror::Error)]
pub enum TurnStoreError {
    #[error("no turn in progress")]
    NoCurrentTurn,
    #[error("current turn is not in progress (outcome already set)")]
    CurrentTurnFinished,
    #[error("turn not found: {0}")]
    TurnNotFound(String),
    #[error("step index out of range: {step_index}")]
    StepIndexOutOfRange { step_index: u32 },
    #[error("step at index {step_index} is not a ToolBatch")]
    StepNotToolBatch { step_index: u32 },
    #[error("tool item index out of range: {item_index} (batch len {batch_len})")]
    ItemIndexOutOfRange { item_index: u32, batch_len: u32 },
}

pub type TurnStoreResult<T> = Result<T, TurnStoreError>;

#[derive(Default)]
pub struct TurnStore {
    pub(super) turns: Vec<Turn>,
    pub(super) current_turn_id: Option<TurnId>,
}

impl TurnStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_turns(turns: Vec<Turn>) -> Self {
        let current_turn_id = turns
            .iter()
            .rev()
            .find(|t| matches!(t.outcome, TurnOutcome::InProgress))
            .map(|t| t.id.clone());
        Self {
            turns,
            current_turn_id,
        }
    }

    pub(crate) fn start_turn(&mut self, trigger: TurnTrigger) -> TurnId {
        let turn = Turn::new(trigger);
        let id = turn.id.clone();
        self.turns.push(turn);
        self.current_turn_id = Some(id.clone());
        id
    }

    pub(crate) fn append_step(&mut self, step: TurnStep) -> TurnStoreResult<u32> {
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
        turn.last_step_at = Some(chrono::Utc::now());
        Ok(idx)
    }

    pub(crate) fn update_tool_state(
        &mut self,
        step_index: u32,
        item_index: u32,
        new_state: ToolExecState,
    ) -> TurnStoreResult<()> {
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
        let step = turn
            .body
            .steps
            .get_mut(step_index as usize)
            .ok_or(TurnStoreError::StepIndexOutOfRange { step_index })?;
        let batch = match step {
            TurnStep::ToolBatch(b) => b,
            _ => return Err(TurnStoreError::StepNotToolBatch { step_index }),
        };
        let batch_len = batch.items.len() as u32;
        let item = batch.items.get_mut(item_index as usize).ok_or(
            TurnStoreError::ItemIndexOutOfRange {
                item_index,
                batch_len,
            },
        )?;
        item.state = new_state;
        turn.last_step_at = Some(chrono::Utc::now());
        Ok(())
    }

    pub(crate) fn end_current_turn(&mut self, outcome: TurnOutcome) -> TurnStoreResult<()> {
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
        turn.outcome = outcome;
        self.current_turn_id = None;
        Ok(())
    }

    pub(crate) fn clear(&mut self) {
        self.turns.clear();
        self.current_turn_id = None;
    }

    pub(crate) fn truncate_turns(&mut self, keep: usize) {
        self.turns.truncate(keep);
        if let Some(id) = &self.current_turn_id
            && !self.turns.iter().any(|t| &t.id == id)
        {
            self.current_turn_id = None;
        }
    }

    /// Replace turn vec. Clears `current_turn_id` if the referenced turn no
    /// longer exists OR is no longer InProgress (Complete turns after replay
    /// must not stay current — next append_step would surface
    /// CurrentTurnFinished instead of NoCurrentTurn).
    pub(crate) fn replace_turns(&mut self, turns: Vec<Turn>) {
        self.turns = turns;
        if let Some(id) = &self.current_turn_id {
            let still_in_progress = self
                .turns
                .iter()
                .find(|t| &t.id == id)
                .is_some_and(|t| matches!(t.outcome, TurnOutcome::InProgress));
            if !still_in_progress {
                self.current_turn_id = None;
            }
        }
    }
}
