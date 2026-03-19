use std::path::Path;

use loopal_storage::{MessageStore, Session, SessionStore};
use loopal_types::error::Result;
use loopal_types::message::Message;
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

    /// Persist a message to the session's message store.
    pub fn save_message(&self, session_id: &str, message: &Message) -> Result<()> {
        self.message_store.append_message(session_id, message)?;
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

