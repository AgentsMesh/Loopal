//! Session query methods — list, filter, and search sessions.

use std::path::Path;

use loopal_error::StorageError;

use super::sessions::{Session, SessionStore, normalize_cwd};

impl SessionStore {
    /// Find the most recently updated session for a given working directory.
    pub fn latest_session_for_cwd(&self, cwd: &Path) -> Result<Option<Session>, StorageError> {
        let cwd_str = normalize_cwd(cwd);
        let mut sessions = self.list_sessions()?;
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions.into_iter().find(|s| s.cwd == cwd_str))
    }

    /// List sessions filtered by working directory, sorted by `updated_at` (newest first).
    pub fn list_sessions_for_cwd(&self, cwd: &Path) -> Result<Vec<Session>, StorageError> {
        let cwd_str = normalize_cwd(cwd);
        let mut sessions: Vec<Session> = self
            .list_sessions()?
            .into_iter()
            .filter(|s| s.cwd == cwd_str)
            .collect();
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    /// List only root (non-sub-agent) sessions for a working directory.
    /// Scans ALL sessions to build the exclusion set, covering cross-cwd sub-agents.
    pub fn list_root_sessions_for_cwd(&self, cwd: &Path) -> Result<Vec<Session>, StorageError> {
        let all = self.list_sessions()?;
        let sub_ids: std::collections::HashSet<String> = all
            .iter()
            .flat_map(|s| s.sub_agents.iter().map(|r| r.session_id.clone()))
            .collect();
        let cwd_str = normalize_cwd(cwd);
        let mut root: Vec<Session> = all
            .into_iter()
            .filter(|s| s.cwd == cwd_str && !sub_ids.contains(&s.id))
            .collect();
        root.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(root)
    }

    /// List all sessions, sorted by creation time (newest first).
    pub fn list_sessions(&self) -> Result<Vec<Session>, StorageError> {
        let sessions_dir = self.sessions_dir();
        if !sessions_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();
        for entry in std::fs::read_dir(&sessions_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let session_file = entry.path().join("session.json");
                if session_file.exists() {
                    let contents = std::fs::read_to_string(&session_file)?;
                    if let Ok(session) = serde_json::from_str::<Session>(&contents) {
                        sessions.push(session);
                    }
                }
            }
        }

        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(sessions)
    }
}
