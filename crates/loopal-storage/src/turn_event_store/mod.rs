mod fold;
mod synthesize;

use std::io::Write;
use std::path::PathBuf;

use loopal_error::StorageError;
use loopal_turn::{Turn, TurnEvent, TurnId};

pub use fold::fold_events;
#[doc(hidden)]
pub use synthesize::{finalize_incomplete_turns, synthesize_missing_tool_batches};

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
        self.append_event_with_durability(session_id, event, false)
    }

    pub fn append_event_durable(
        &self,
        session_id: &str,
        event: &TurnEvent,
    ) -> Result<(), StorageError> {
        self.append_event_with_durability(session_id, event, true)
    }

    fn append_event_with_durability(
        &self,
        session_id: &str,
        event: &TurnEvent,
        durable: bool,
    ) -> Result<(), StorageError> {
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
        if durable {
            file.flush()?;
            file.sync_data()?;
        }
        Ok(())
    }

    pub fn load_events(&self, session_id: &str) -> Result<Vec<TurnEvent>, StorageError> {
        let path = self.turns_file(session_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let contents = read_repairing_torn_tail(&path)?;
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

    pub fn sync_events(&self, session_id: &str) -> Result<(), StorageError> {
        let path = self.turns_file(session_id);
        let file = std::fs::OpenOptions::new().append(true).open(path)?;
        file.sync_data()?;
        Ok(())
    }

    pub fn load_turns(&self, session_id: &str) -> Result<Vec<Turn>, StorageError> {
        let events = self.load_events(session_id)?;
        Ok(fold_events(events))
    }
}

fn read_repairing_torn_tail(path: &std::path::Path) -> Result<String, StorageError> {
    let mut bytes = std::fs::read(path)?;
    if !bytes.is_empty() && bytes.last() != Some(&b'\n') {
        let complete_len = bytes
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |index| index + 1);
        let file = std::fs::OpenOptions::new().write(true).open(path)?;
        file.set_len(complete_len as u64)?;
        file.sync_data()?;
        bytes.truncate(complete_len);
        tracing_warn("discarded an incomplete final turn-event record");
    }
    String::from_utf8(bytes).map_err(|error| StorageError::Serialization(error.to_string()))
}

fn tracing_warn(msg: &str) {
    eprintln!("[loopal-storage] {msg}");
}

pub fn turn_id_from_str(s: &str) -> TurnId {
    TurnId::from_string(s)
}
