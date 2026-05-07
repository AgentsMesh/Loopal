use std::path::PathBuf;

use loopal_error::StorageError;
use loopal_protocol::ThreadGoal;

/// Persists at most one [`ThreadGoal`] per session as a JSON file inside the
/// existing session directory: `<base_dir>/sessions/<session_id>/goal.json`.
///
/// Writes go through a `.tmp + rename` pair so a crashed process never leaves
/// a half-written file. Missing files are reported as `Ok(None)`, never as
/// errors — having no goal is the default state.
pub struct GoalStore {
    base_dir: PathBuf,
}

impl GoalStore {
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub fn from_default_dir() -> Result<Self, StorageError> {
        let base_dir =
            loopal_config::global_config_dir().map_err(|_| StorageError::HomeDirNotFound)?;
        Ok(Self { base_dir })
    }

    fn session_dir(&self, session_id: &str) -> PathBuf {
        self.base_dir.join("sessions").join(session_id)
    }

    fn goal_file(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("goal.json")
    }

    pub fn load(&self, session_id: &str) -> Result<Option<ThreadGoal>, StorageError> {
        let path = self.goal_file(session_id);
        match std::fs::read_to_string(&path) {
            Ok(contents) => {
                let goal: ThreadGoal = serde_json::from_str(&contents)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok(Some(goal))
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(StorageError::Io(err)),
        }
    }

    pub fn save(&self, goal: &ThreadGoal) -> Result<(), StorageError> {
        let dir = self.session_dir(&goal.session_id);
        std::fs::create_dir_all(&dir)?;

        let json = serde_json::to_string_pretty(goal)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        let target = self.goal_file(&goal.session_id);
        let tmp = dir.join(".goal.json.tmp");
        std::fs::write(&tmp, json)?;
        // reason: rename is atomic on the same filesystem; readers either see
        // the previous goal.json or the new one, never a half-written file.
        std::fs::rename(&tmp, &target)?;
        Ok(())
    }

    pub fn clear(&self, session_id: &str) -> Result<(), StorageError> {
        let path = self.goal_file(session_id);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(StorageError::Io(err)),
        }
    }
}
