use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use loopal_error::StorageError;

/// Reference to a sub-agent session spawned during a parent session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentRef {
    /// Display name of the sub-agent.
    pub name: String,
    /// Session ID of the sub-agent's own session storage.
    pub session_id: String,
    /// Parent agent name (None for agents spawned by root).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Model used by the sub-agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Session metadata persisted to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub model: String,
    pub cwd: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub mode: String,
    /// Sub-agent sessions spawned during this session.
    #[serde(default)]
    pub sub_agents: Vec<SubAgentRef>,
}

/// File-based session store.
/// Sessions are stored at `<base_dir>/sessions/<id>/session.json`.
pub struct SessionStore {
    base_dir: PathBuf,
}

impl SessionStore {
    /// Create a store using the default global directory (~/.loopal).
    pub fn new() -> Result<Self, StorageError> {
        let base_dir =
            loopal_config::global_config_dir().map_err(|_| StorageError::HomeDirNotFound)?;
        Ok(Self { base_dir })
    }

    /// Create a store with a custom base directory (useful for testing).
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub(crate) fn sessions_dir(&self) -> PathBuf {
        self.base_dir.join("sessions")
    }

    fn session_dir(&self, session_id: &str) -> PathBuf {
        self.sessions_dir().join(session_id)
    }

    fn session_file(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("session.json")
    }

    /// Create a new session and persist it.
    pub fn create_session(&self, cwd: &Path, model: &str) -> Result<Session, StorageError> {
        let now = Utc::now();
        let session = Session {
            id: Uuid::new_v4().to_string(),
            title: String::new(),
            model: model.to_string(),
            cwd: normalize_cwd(cwd),
            created_at: now,
            updated_at: now,
            mode: "default".to_string(),
            sub_agents: Vec::new(),
        };

        let dir = self.session_dir(&session.id);
        std::fs::create_dir_all(&dir)?;

        let json = serde_json::to_string_pretty(&session)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        std::fs::write(self.session_file(&session.id), json)?;

        Ok(session)
    }

    /// Load an existing session by ID.
    pub fn load_session(&self, session_id: &str) -> Result<Session, StorageError> {
        let path = self.session_file(session_id);
        let contents = std::fs::read_to_string(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StorageError::SessionNotFound(session_id.to_string())
            } else {
                StorageError::Io(e)
            }
        })?;
        let session: Session = serde_json::from_str(&contents)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        Ok(session)
    }

    /// Update a session on disk.
    pub fn update_session(&self, session: &Session) -> Result<(), StorageError> {
        let dir = self.session_dir(&session.id);
        if !dir.exists() {
            return Err(StorageError::SessionNotFound(session.id.clone()));
        }

        let json = serde_json::to_string_pretty(session)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        std::fs::write(self.session_file(&session.id), json)?;
        Ok(())
    }

    /// Record a sub-agent reference in the parent session and persist.
    pub fn add_sub_agent(
        &self,
        parent_session_id: &str,
        sub_ref: SubAgentRef,
    ) -> Result<(), StorageError> {
        let mut session = self.load_session(parent_session_id)?;
        // Avoid duplicates (same sub-agent name + session_id).
        if !session
            .sub_agents
            .iter()
            .any(|s| s.session_id == sub_ref.session_id)
        {
            session.sub_agents.push(sub_ref);
            self.update_session(&session)?;
        }
        Ok(())
    }
}

/// Canonicalize a path for consistent session cwd comparison.
/// Falls back to the original path if canonicalization fails (e.g. path doesn't exist yet).
pub(crate) fn normalize_cwd(cwd: &Path) -> String {
    std::fs::canonicalize(cwd)
        .unwrap_or_else(|_| cwd.to_path_buf())
        .to_string_lossy()
        .to_string()
}
