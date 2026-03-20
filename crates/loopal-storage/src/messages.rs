use std::io::Write;
use std::path::PathBuf;

use loopal_error::StorageError;
use loopal_message::Message;

use crate::entry::TaggedEntry;
use crate::replay;

/// File-based message store using JSONL format.
/// Each line is a `TaggedEntry` (message or marker).
/// Stored at `<base_dir>/sessions/<id>/messages.jsonl`.
pub struct MessageStore {
    base_dir: PathBuf,
}

impl MessageStore {
    /// Create a store using the default global directory (~/.loopal).
    pub fn new() -> Result<Self, StorageError> {
        let base_dir = loopal_config::global_config_dir()
            .map_err(|_| StorageError::HomeDirNotFound)?;
        Ok(Self { base_dir })
    }

    /// Create a store with a custom base directory (useful for testing).
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    fn messages_file(&self, session_id: &str) -> PathBuf {
        self.base_dir
            .join("sessions")
            .join(session_id)
            .join("messages.jsonl")
    }

    fn append_line(&self, session_id: &str, line: &str) -> Result<(), StorageError> {
        let path = self.messages_file(session_id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        writeln!(file, "{}", line)?;
        Ok(())
    }

    /// Append any entry (message or marker) to the JSONL file.
    pub fn append_entry(
        &self,
        session_id: &str,
        entry: &TaggedEntry,
    ) -> Result<(), StorageError> {
        let line = serde_json::to_string(entry)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.append_line(session_id, &line)
    }

    /// Convenience: append a message as a `TaggedEntry::Message`.
    pub fn append_message(
        &self,
        session_id: &str,
        message: &Message,
    ) -> Result<(), StorageError> {
        self.append_entry(session_id, &TaggedEntry::Message(message.clone()))
    }

    /// Load raw entries without replay (useful for debugging).
    pub fn load_entries(
        &self,
        session_id: &str,
    ) -> Result<Vec<TaggedEntry>, StorageError> {
        let path = self.messages_file(session_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let contents = std::fs::read_to_string(&path)?;
        let mut entries = Vec::new();
        for line in contents.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let entry: TaggedEntry = serde_json::from_str(line)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            entries.push(entry);
        }
        Ok(entries)
    }

    /// Load messages for a session, replaying any markers (Clear/CompactTo/RewindTo).
    pub fn load_messages(
        &self,
        session_id: &str,
    ) -> Result<Vec<Message>, StorageError> {
        let entries = self.load_entries(session_id)?;
        Ok(replay::replay(entries))
    }
}
