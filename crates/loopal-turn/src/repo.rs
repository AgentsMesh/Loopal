use std::sync::{Arc, RwLock};

use crate::event::TurnEvent;
use crate::step::{ToolExecState, TurnStep};
use crate::turn::{Turn, TurnId, TurnOutcome, TurnTrigger};

#[derive(Debug, thiserror::Error)]
pub enum TurnRepoError {
    #[error("turn not found: {0}")]
    TurnNotFound(String),
    #[error("turn already ended: {0}")]
    TurnAlreadyEnded(String),
    #[error("step index out of range: turn={turn} idx={idx}")]
    StepIndexOutOfRange { turn: String, idx: u32 },
    #[error("item index out of range: turn={turn} step={step} item={item}")]
    ItemIndexOutOfRange { turn: String, step: u32, item: u32 },
    #[error("step is not a ToolBatch (cannot update item state)")]
    StepNotToolBatch,
    #[error("storage error: {0}")]
    Storage(String),
}

pub type TurnRepoResult<T> = Result<T, TurnRepoError>;

pub trait TurnRepo: Send + Sync {
    fn start_turn(&self, trigger: TurnTrigger) -> TurnRepoResult<TurnId>;

    fn append_step(&self, turn_id: &TurnId, step: TurnStep) -> TurnRepoResult<u32>;

    fn update_tool_state(
        &self,
        turn_id: &TurnId,
        step_index: u32,
        item_index: u32,
        new_state: ToolExecState,
    ) -> TurnRepoResult<()>;

    fn end_turn(&self, turn_id: &TurnId, outcome: TurnOutcome) -> TurnRepoResult<()>;

    fn load_turns(&self) -> TurnRepoResult<Vec<Turn>>;

    fn snapshot_turn(&self, turn_id: &TurnId) -> TurnRepoResult<Turn>;
}

#[derive(Default)]
pub struct InMemoryTurnRepo {
    state: Arc<RwLock<InMemoryState>>,
}

#[derive(Default)]
struct InMemoryState {
    turns: Vec<Turn>,
    events: Vec<TurnEvent>,
}

impl InMemoryTurnRepo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> Vec<TurnEvent> {
        self.state.read().unwrap().events.clone()
    }
}

impl TurnRepo for InMemoryTurnRepo {
    fn start_turn(&self, trigger: TurnTrigger) -> TurnRepoResult<TurnId> {
        let turn = Turn::new(trigger.clone());
        let id = turn.id.clone();
        let mut st = self.state.write().unwrap();
        st.events.push(TurnEvent::TurnStarted {
            turn_id: id.clone(),
            started_at: turn.started_at,
            trigger,
        });
        st.turns.push(turn);
        Ok(id)
    }

    fn append_step(&self, turn_id: &TurnId, step: TurnStep) -> TurnRepoResult<u32> {
        let mut st = self.state.write().unwrap();
        let turn = find_turn_mut(&mut st.turns, turn_id)?;
        ensure_in_progress(turn)?;
        let idx = turn.body.steps.len() as u32;
        turn.body.steps.push(step.clone());
        st.events.push(TurnEvent::StepAppended {
            turn_id: turn_id.clone(),
            step_index: idx,
            step,
            appended_at: Some(chrono::Utc::now()),
        });
        Ok(idx)
    }

    fn update_tool_state(
        &self,
        turn_id: &TurnId,
        step_index: u32,
        item_index: u32,
        new_state: ToolExecState,
    ) -> TurnRepoResult<()> {
        let mut st = self.state.write().unwrap();
        let turn = find_turn_mut(&mut st.turns, turn_id)?;
        let step = turn
            .body
            .steps
            .get_mut(step_index as usize)
            .ok_or_else(|| TurnRepoError::StepIndexOutOfRange {
                turn: turn_id.as_str().to_string(),
                idx: step_index,
            })?;
        let TurnStep::ToolBatch(batch) = step else {
            return Err(TurnRepoError::StepNotToolBatch);
        };
        let item = batch.items.get_mut(item_index as usize).ok_or_else(|| {
            TurnRepoError::ItemIndexOutOfRange {
                turn: turn_id.as_str().to_string(),
                step: step_index,
                item: item_index,
            }
        })?;
        item.state = new_state.clone();
        st.events.push(TurnEvent::StepUpdated {
            turn_id: turn_id.clone(),
            step_index,
            item_index,
            new_state,
            updated_at: Some(chrono::Utc::now()),
        });
        Ok(())
    }

    fn end_turn(&self, turn_id: &TurnId, outcome: TurnOutcome) -> TurnRepoResult<()> {
        let mut st = self.state.write().unwrap();
        let turn = find_turn_mut(&mut st.turns, turn_id)?;
        ensure_in_progress(turn)?;
        turn.outcome = outcome.clone();
        st.events.push(TurnEvent::TurnEnded {
            turn_id: turn_id.clone(),
            outcome,
        });
        Ok(())
    }

    fn load_turns(&self) -> TurnRepoResult<Vec<Turn>> {
        Ok(self.state.read().unwrap().turns.clone())
    }

    fn snapshot_turn(&self, turn_id: &TurnId) -> TurnRepoResult<Turn> {
        let st = self.state.read().unwrap();
        st.turns
            .iter()
            .find(|t| &t.id == turn_id)
            .cloned()
            .ok_or_else(|| TurnRepoError::TurnNotFound(turn_id.as_str().to_string()))
    }
}

fn find_turn_mut<'a>(turns: &'a mut [Turn], id: &TurnId) -> TurnRepoResult<&'a mut Turn> {
    turns
        .iter_mut()
        .find(|t| &t.id == id)
        .ok_or_else(|| TurnRepoError::TurnNotFound(id.as_str().to_string()))
}

fn ensure_in_progress(turn: &Turn) -> TurnRepoResult<()> {
    if !matches!(turn.outcome, TurnOutcome::InProgress) {
        return Err(TurnRepoError::TurnAlreadyEnded(
            turn.id.as_str().to_string(),
        ));
    }
    Ok(())
}
