use std::io::Write;
use std::path::PathBuf;

use loopal_error::StorageError;
use loopal_turn::{Turn, TurnBody, TurnEvent, TurnId, TurnOutcome, TurnStep};

pub struct TurnEventStore {
    base_dir: PathBuf,
}

impl TurnEventStore {
    pub fn new() -> Result<Self, StorageError> {
        let base_dir =
            loopal_config::global_config_dir().map_err(|_| StorageError::HomeDirNotFound)?;
        Ok(Self { base_dir })
    }

    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    fn turns_file(&self, session_id: &str) -> PathBuf {
        self.base_dir
            .join("sessions")
            .join(session_id)
            .join("turns.jsonl")
    }

    pub fn append_event(&self, session_id: &str, event: &TurnEvent) -> Result<(), StorageError> {
        let line =
            serde_json::to_string(event).map_err(|e| StorageError::Serialization(e.to_string()))?;
        let path = self.turns_file(session_id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        writeln!(file, "{line}")?;
        Ok(())
    }

    pub fn load_events(&self, session_id: &str) -> Result<Vec<TurnEvent>, StorageError> {
        let path = self.turns_file(session_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let contents = std::fs::read_to_string(&path)?;
        let mut events = Vec::new();
        for line in contents.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let event: TurnEvent = serde_json::from_str(line)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            events.push(event);
        }
        Ok(events)
    }

    pub fn load_turns(&self, session_id: &str) -> Result<Vec<Turn>, StorageError> {
        let events = self.load_events(session_id)?;
        Ok(fold_events(events))
    }
}

pub fn fold_events(events: Vec<TurnEvent>) -> Vec<Turn> {
    let mut turns: Vec<Turn> = Vec::new();
    for event in events {
        match event {
            TurnEvent::TurnStarted {
                turn_id,
                started_at,
                trigger,
            } => {
                turns.push(Turn {
                    id: turn_id,
                    started_at,
                    trigger,
                    body: TurnBody::default(),
                    outcome: TurnOutcome::InProgress,
                });
            }
            TurnEvent::StepAppended {
                turn_id,
                step_index,
                step,
            } => {
                if let Some(turn) = turns.iter_mut().find(|t| t.id == turn_id) {
                    apply_step_append(&mut turn.body.steps, step_index, step);
                }
            }
            TurnEvent::StepUpdated {
                turn_id,
                step_index,
                item_index,
                new_state,
            } => {
                if let Some(turn) = turns.iter_mut().find(|t| t.id == turn_id) {
                    apply_step_update(&mut turn.body.steps, step_index, item_index, new_state);
                }
            }
            TurnEvent::TurnEnded { turn_id, outcome } => {
                if let Some(turn) = turns.iter_mut().find(|t| t.id == turn_id) {
                    turn.outcome = outcome;
                }
            }
        }
    }
    finalize_incomplete_turns(&mut turns);
    turns
}

fn apply_step_append(steps: &mut Vec<TurnStep>, step_index: u32, step: TurnStep) {
    let idx = step_index as usize;
    if idx == steps.len() {
        steps.push(step);
    } else if idx < steps.len() {
        steps[idx] = step;
    } else {
        // reason: 索引越界 = jsonl 损坏，跳过但不 panic 保护 load 路径。
        tracing_warn(&format!(
            "StepAppended index {idx} out of range (len={}); dropping",
            steps.len()
        ));
    }
}

fn apply_step_update(
    steps: &mut [TurnStep],
    step_index: u32,
    item_index: u32,
    new_state: loopal_turn::ToolExecState,
) {
    let sidx = step_index as usize;
    let iidx = item_index as usize;
    if let Some(TurnStep::ToolBatch(batch)) = steps.get_mut(sidx)
        && let Some(item) = batch.items.get_mut(iidx)
    {
        item.state = new_state;
    }
}

fn finalize_incomplete_turns(turns: &mut [Turn]) {
    for turn in turns.iter_mut() {
        if !matches!(turn.outcome, TurnOutcome::InProgress) {
            continue;
        }
        // reason: 缺 TurnEnded → crash recovery；按 Cancelled 收口，并把所有 in-flight
        // tool item 标 Cancelled，确保 fold 出的 Vec<Turn> 不会破坏后续 invariant。
        turn.outcome = TurnOutcome::Cancelled {
            cause: loopal_turn::CancelledCause::CrashRecovery,
        };
        for step in turn.body.steps.iter_mut() {
            if let TurnStep::ToolBatch(batch) = step {
                for item in batch.items.iter_mut() {
                    if matches!(
                        item.state,
                        loopal_turn::ToolExecState::Pending | loopal_turn::ToolExecState::Running
                    ) {
                        item.state = loopal_turn::ToolExecState::Cancelled(
                            loopal_turn::CancelCause::CrashRecovery,
                        );
                    }
                }
            }
        }
    }
}

fn tracing_warn(msg: &str) {
    // reason: storage crate 没引入 tracing；用 eprintln 在 stderr 留痕。
    eprintln!("[loopal-storage] {msg}");
}

pub fn turn_id_from_str(s: &str) -> TurnId {
    TurnId::from_string(s)
}
