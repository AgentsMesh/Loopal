use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::json_file_io::{quarantine_path, read_or_empty, write_atomic};
use crate::persistence::{
    LoadedPayload, PersistError, PersistedTask, classify_payload, encode_payload,
};
use crate::persistence_session::SessionScopedCronStorage;

/// Path layout: `<sessions_root>/<session_id>/cron.json`. Reads of
/// missing data return an empty list so fresh sessions start clean.
pub struct FileScopedCronStore {
    sessions_root: PathBuf,
}

impl FileScopedCronStore {
    pub fn new(sessions_root: PathBuf) -> Self {
        Self { sessions_root }
    }

    pub fn root(&self) -> &Path {
        &self.sessions_root
    }

    fn path_for(&self, session_id: &str) -> PathBuf {
        self.sessions_root.join(session_id).join("cron.json")
    }
}

#[async_trait]
impl SessionScopedCronStorage for FileScopedCronStore {
    async fn load(&self, session_id: &str) -> Result<Vec<PersistedTask>, PersistError> {
        let path = self.path_for(session_id);
        let bytes = read_or_empty(&path).await?;
        match classify_payload(&bytes) {
            LoadedPayload::Empty => Ok(Vec::new()),
            LoadedPayload::Tasks(t) => Ok(t),
            LoadedPayload::Quarantine(reason) => {
                quarantine_path(&path, &reason).await?;
                Ok(Vec::new())
            }
        }
    }

    async fn save_all(
        &self,
        session_id: &str,
        tasks: &[PersistedTask],
    ) -> Result<(), PersistError> {
        let bytes = encode_payload(tasks)?;
        write_atomic(&self.path_for(session_id), &bytes).await
    }
}
