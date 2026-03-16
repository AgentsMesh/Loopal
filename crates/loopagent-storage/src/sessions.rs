use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use loopagent_types::error::StorageError;

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
}

/// File-based session store.
/// Sessions are stored at `<base_dir>/sessions/<id>/session.json`.
pub struct SessionStore {
    base_dir: PathBuf,
}

impl SessionStore {
    /// Create a store using the default global directory (~/.loopagent).
    pub fn new() -> Result<Self, StorageError> {
        let base_dir = dirs::home_dir()
            .ok_or(StorageError::HomeDirNotFound)?
            .join(".loopagent");
        Ok(Self { base_dir })
    }

    /// Create a store with a custom base directory (useful for testing).
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    fn sessions_dir(&self) -> PathBuf {
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
            cwd: cwd.to_string_lossy().to_string(),
            created_at: now,
            updated_at: now,
            mode: "default".to_string(),
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