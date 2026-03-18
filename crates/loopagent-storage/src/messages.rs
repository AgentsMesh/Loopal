use std::io::Write;
use std::path::PathBuf;

use loopagent_types::error::StorageError;
use loopagent_types::message::Message;

/// File-based message store using JSONL format.
/// Messages are stored at `<base_dir>/sessions/<id>/messages.jsonl`.
pub struct MessageStore {
    base_dir: PathBuf,
}

impl MessageStore {
    /// Create a store using the default global directory (~/.loopagent).
    pub fn new() -> Result<Self, StorageError> {
        let base_dir = loopagent_config::global_config_dir()
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

    /// Append a message to the session's JSONL file.
    pub fn append_message(&self, session_id: &str, message: &Message) -> Result<(), StorageError> {
        let path = self.messages_file(session_id);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let line = serde_json::to_string(message)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        writeln!(file, "{}", line)?;

        Ok(())
    }

    /// Load all messages for a session from the JSONL file.
    pub fn load_messages(&self, session_id: &str) -> Result<Vec<Message>, StorageError> {
        let path = self.messages_file(session_id);

        if !path.exists() {
            return Ok(Vec::new());
        }

        let contents = std::fs::read_to_string(&path)?;
        let mut messages = Vec::new();

        for line in contents.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let message: Message = serde_json::from_str(line)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            messages.push(message);
        }

        Ok(messages)
    }
}