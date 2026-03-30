use std::path::Path;

use loopal_error::Result;
use loopal_message::Message;
use loopal_storage::entry::{Marker, TaggedEntry};
use loopal_storage::{MessageStore, Session, SessionStore, SubAgentRef};
use tracing::info;

/// Manages session creation, resumption, and message persistence.
pub struct SessionManager {
    session_store: SessionStore,
    message_store: MessageStore,
}

impl SessionManager {
    pub fn new() -> Result<Self> {
        Ok(Self {
            session_store: SessionStore::new()?,
            message_store: MessageStore::new()?,
        })
    }

    /// Create a SessionManager backed by a custom base directory.
    /// This is primarily useful for testing with temp directories.
    pub fn with_base_dir(base_dir: std::path::PathBuf) -> Self {
        Self {
            session_store: SessionStore::with_base_dir(base_dir.clone()),
            message_store: MessageStore::with_base_dir(base_dir),
        }
    }

    /// Create a new session.
    pub fn create_session(&self, cwd: &Path, model: &str) -> Result<Session> {
        let session = self.session_store.create_session(cwd, model)?;
        info!(session_id = %session.id, model = %model, cwd = %cwd.display(), "session created");
        Ok(session)
    }

    /// Resume an existing session by loading it and its messages.
    pub fn resume_session(&self, session_id: &str) -> Result<(Session, Vec<Message>)> {
        let session = self.session_store.load_session(session_id)?;
        let messages = self.message_store.load_messages(session_id)?;
        info!(session_id = %session_id, message_count = messages.len(), "session resumed");
        Ok((session, messages))
    }

    /// Load messages for a sub-agent session (by session_id).
    pub fn load_messages(&self, session_id: &str) -> Result<Vec<Message>> {
        let messages = self.message_store.load_messages(session_id)?;
        Ok(messages)
    }

    /// Record a sub-agent reference in the parent session.
    pub fn add_sub_agent(&self, parent_session_id: &str, sub_ref: SubAgentRef) -> Result<()> {
        self.session_store
            .add_sub_agent(parent_session_id, sub_ref)?;
        Ok(())
    }

    /// Persist a message to the session's message store.
    /// Automatically assigns a UUID in-place if the message has no id,
    /// so the caller's copy stays in sync with storage.
    pub fn save_message(&self, session_id: &str, message: &mut Message) -> Result<()> {
        if message.id.is_none() {
            message.id = Some(uuid::Uuid::new_v4().to_string());
        }
        self.message_store.append_message(session_id, message)?;
        Ok(())
    }

    /// Append a Clear marker to the event log.
    /// On next load, all messages before this marker are discarded.
    pub fn clear_history(&self, session_id: &str) -> Result<()> {
        let entry = TaggedEntry::Marker(Marker::Clear {
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
        self.message_store.append_entry(session_id, &entry)?;
        info!(session_id = %session_id, "clear marker written");
        Ok(())
    }

    /// Append a CompactTo marker to the event log.
    /// On next load, only the last `keep_last` messages are retained.
    pub fn compact_history(&self, session_id: &str, keep_last: usize) -> Result<()> {
        let entry = TaggedEntry::Marker(Marker::CompactTo {
            keep_last,
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
        self.message_store.append_entry(session_id, &entry)?;
        info!(session_id = %session_id, keep_last, "compact marker written");
        Ok(())
    }

    /// Append a RewindTo marker to the event log.
    /// On next load, the message with `message_id` and everything after it are discarded.
    pub fn rewind_to(&self, session_id: &str, message_id: &str) -> Result<()> {
        let entry = TaggedEntry::Marker(Marker::RewindTo {
            message_id: message_id.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
        self.message_store.append_entry(session_id, &entry)?;
        info!(session_id = %session_id, message_id = %message_id, "rewind marker written");
        Ok(())
    }

    /// Update session metadata.
    pub fn update_session(&self, session: &Session) -> Result<()> {
        self.session_store.update_session(session)?;
        Ok(())
    }

    /// List all sessions.
    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        let sessions = self.session_store.list_sessions()?;
        Ok(sessions)
    }
}
