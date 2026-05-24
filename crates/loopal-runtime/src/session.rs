use std::path::Path;
use std::sync::Arc;

use loopal_error::Result;
use loopal_provider_api::Message;
use loopal_storage::entry::{Marker, TaggedEntry};
use loopal_storage::{GoalStore, MessageStore, Session, SessionStore, SubAgentRef, TurnEventStore};
use loopal_turn::TurnEvent;
use tracing::info;

use crate::legacy_message_to_turn::legacy_messages_to_turns;

/// Manages session creation, resumption, and message persistence.
pub struct SessionManager {
    session_store: SessionStore,
    message_store: MessageStore,
    turn_event_store: TurnEventStore,
    goal_store: Arc<GoalStore>,
}

impl SessionManager {
    pub fn new() -> Result<Self> {
        Ok(Self {
            session_store: SessionStore::new()?,
            message_store: MessageStore::new()?,
            turn_event_store: TurnEventStore::new()?,
            goal_store: Arc::new(GoalStore::from_default_dir()?),
        })
    }

    /// Create a SessionManager backed by a custom base directory.
    /// This is primarily useful for testing with temp directories.
    pub fn with_base_dir(base_dir: std::path::PathBuf) -> Self {
        Self {
            session_store: SessionStore::with_base_dir(base_dir.clone()),
            message_store: MessageStore::with_base_dir(base_dir.clone()),
            turn_event_store: TurnEventStore::with_base_dir(base_dir.clone()),
            goal_store: Arc::new(GoalStore::with_base_dir(base_dir)),
        }
    }

    pub fn goal_store(&self) -> Arc<GoalStore> {
        Arc::clone(&self.goal_store)
    }

    /// Create a new session.
    pub fn create_session(&self, cwd: &Path, model: &str) -> Result<Session> {
        let session = self.session_store.create_session(cwd, model)?;
        info!(session_id = %session.id, model = %model, cwd = %cwd.display(), "session created");
        Ok(session)
    }

    pub fn resume_session(
        &self,
        session_id: &str,
    ) -> Result<(Session, Vec<loopal_turn::Turn>, Vec<Message>)> {
        let session = self.session_store.load_session(session_id)?;
        let turns_from_log = self.turn_event_store.load_turns(session_id)?;
        let (turns, messages) = if turns_from_log.is_empty() {
            let legacy = self.message_store.load_messages(session_id)?;
            (legacy_messages_to_turns(legacy.clone()), legacy)
        } else {
            let projected = loopal_provider_api::project_turns_to_messages(&turns_from_log);
            (turns_from_log, projected)
        };
        info!(
            session_id = %session_id,
            message_count = messages.len(),
            turn_count = turns.len(),
            "session resumed"
        );
        Ok((session, turns, messages))
    }

    /// Load messages for a sub-agent session (by session_id).
    /// Same fallback semantics as `resume_session`.
    pub fn load_messages(&self, session_id: &str) -> Result<Vec<Message>> {
        let turns = self.turn_event_store.load_turns(session_id)?;
        if !turns.is_empty() {
            return Ok(loopal_provider_api::project_turns_to_messages(&turns));
        }
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

    /// Persist a Turn-domain event to the session's turn event log.
    /// turns.jsonl is the new SSOT under construction; messages.jsonl
    /// remains as the active read path until PR-6.
    pub fn record_turn_event(&self, session_id: &str, event: &TurnEvent) -> Result<()> {
        self.turn_event_store
            .append_event(session_id, event)
            .map_err(loopal_error::LoopalError::from)?;
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

    /// Append a CompactBoundary marker to the event log.
    /// On next load, every message before `summary_msg_id` is dropped,
    /// keeping the summary message and everything after it.
    pub fn mark_compact_boundary(&self, session_id: &str, summary_msg_id: &str) -> Result<()> {
        let entry = TaggedEntry::Marker(Marker::CompactBoundary {
            summary_msg_id: summary_msg_id.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
        self.message_store.append_entry(session_id, &entry)?;
        info!(
            session_id = %session_id,
            summary_msg_id = %summary_msg_id,
            "compact boundary marker written"
        );
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

    /// Find the most recently updated session for a given working directory.
    pub fn latest_session_for_cwd(&self, cwd: &Path) -> Result<Option<Session>> {
        let session = self.session_store.latest_session_for_cwd(cwd)?;
        Ok(session)
    }

    /// List sessions for a given working directory, sorted by `updated_at` (newest first).
    pub fn list_sessions_for_cwd(&self, cwd: &Path) -> Result<Vec<Session>> {
        let sessions = self.session_store.list_sessions_for_cwd(cwd)?;
        Ok(sessions)
    }

    /// List root (non-sub-agent) sessions for a working directory, newest first.
    pub fn list_root_sessions_for_cwd(&self, cwd: &Path) -> Result<Vec<Session>> {
        let sessions = self.session_store.list_root_sessions_for_cwd(cwd)?;
        Ok(sessions)
    }

    /// List all sessions.
    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        let sessions = self.session_store.list_sessions()?;
        Ok(sessions)
    }
}
