use std::path::Path;
use std::sync::Arc;

use loopal_error::Result;
use loopal_storage::{GoalStore, Session, SessionStore, SubAgentRef, TurnEventStore};
use loopal_turn::TurnEvent;
use tracing::info;

/// Manages session creation, resumption, and message persistence.
pub struct SessionManager {
    session_store: SessionStore,
    turn_event_store: TurnEventStore,
    goal_store: Arc<GoalStore>,
}

impl SessionManager {
    pub fn new() -> Result<Self> {
        Ok(Self {
            session_store: SessionStore::new()?,
            turn_event_store: TurnEventStore::new()?,
            goal_store: Arc::new(GoalStore::from_default_dir()?),
        })
    }

    /// Create a SessionManager backed by a custom base directory.
    /// This is primarily useful for testing with temp directories.
    pub fn with_base_dir(base_dir: std::path::PathBuf) -> Self {
        Self {
            session_store: SessionStore::with_base_dir(base_dir.clone()),
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

    pub fn resume_session(&self, session_id: &str) -> Result<(Session, Vec<loopal_turn::Turn>)> {
        let session = self.session_store.load_session(session_id)?;
        let turns = self.turn_event_store.load_turns(session_id)?;
        info!(
            session_id = %session_id,
            turn_count = turns.len(),
            "session resumed"
        );
        Ok((session, turns))
    }

    /// Load sub-agent turns. Caller projects to wire messages on demand
    /// via `loopal_provider_api::project_turns_to_messages`.
    pub fn load_turns(&self, session_id: &str) -> Result<Vec<loopal_turn::Turn>> {
        Ok(self.turn_event_store.load_turns(session_id)?)
    }

    /// Record a sub-agent reference in the parent session.
    pub fn add_sub_agent(&self, parent_session_id: &str, sub_ref: SubAgentRef) -> Result<()> {
        self.session_store
            .add_sub_agent(parent_session_id, sub_ref)?;
        Ok(())
    }

    /// Persist a Turn-domain event to the session's turn event log.
    pub fn record_turn_event(&self, session_id: &str, event: &TurnEvent) -> Result<()> {
        self.turn_event_store
            .append_event(session_id, event)
            .map_err(loopal_error::LoopalError::from)?;
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
