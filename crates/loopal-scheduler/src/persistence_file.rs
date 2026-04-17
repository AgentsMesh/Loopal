//! File-backed [`DurableStore`] with atomic JSON writes.
//!
//! Writes go to `<path>.tmp`, are fsync'd, then renamed over the real
//! path. POSIX guarantees rename is atomic within the same filesystem,
//! so readers always see either the previous or the new contents.
//!
//! When the on-disk payload is structurally invalid (bad JSON or a
//! schema version we can't interpret), `load` **quarantines** the file
//! by renaming it to `<path>.bad-<unix-ms>` and returns `Ok(vec![])`.
//! This avoids silently overwriting a user's durable state the moment
//! they upgrade into an incompatible schema.

use async_trait::async_trait;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

use crate::persistence::{
    DurableStore, PersistError, PersistedFile, PersistedTask, SCHEMA_VERSION,
};

/// Single-file JSON durable store using atomic rename for writes.
///
/// The parent directory is created on first save. Reads of a missing
/// file return an empty list so fresh sessions start cleanly.
pub struct FileDurableStore {
    path: PathBuf,
}

impl FileDurableStore {
    /// Create a store backed by `path`. The file need not exist; the
    /// parent directory is created on first [`save_all`](DurableStore::save_all).
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Inspect the target path (for tests / diagnostics).
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn tmp_path(&self) -> PathBuf {
        self.sibling_with_extension(".tmp")
    }

    fn sibling_with_extension(&self, suffix: &str) -> PathBuf {
        let mut tmp = self.path.clone();
        let name = tmp
            .file_name()
            .map(|n| {
                let mut s = n.to_os_string();
                s.push(suffix);
                s
            })
            .unwrap_or_else(|| OsString::from(format!("cron.json{suffix}")));
        tmp.set_file_name(name);
        tmp
    }

    /// Move a corrupt payload aside so the next save starts from
    /// empty without overwriting the user's original data. Returns
    /// `Ok(())` when the rename succeeds; on failure the corrupt file
    /// is still in place and the caller must refuse further writes to
    /// avoid clobbering it.
    async fn quarantine(&self, reason: &str) -> Result<(), PersistError> {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let dest = self.sibling_with_extension(&format!(".bad-{ts}"));
        match tokio::fs::rename(&self.path, &dest).await {
            Ok(()) => {
                tracing::warn!(
                    from = %self.path.display(),
                    to = %dest.display(),
                    reason,
                    "quarantined corrupt durable cron file"
                );
                Ok(())
            }
            Err(e) => {
                tracing::error!(
                    path = %self.path.display(),
                    error = %e,
                    reason,
                    "failed to quarantine corrupt durable cron file — scheduler will refuse to persist until resolved"
                );
                Err(PersistError::Io(e))
            }
        }
    }
}

#[async_trait]
impl DurableStore for FileDurableStore {
    async fn load(&self) -> Result<Vec<PersistedTask>, PersistError> {
        let bytes = match tokio::fs::read(&self.path).await {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(PersistError::Io(e)),
        };
        match serde_json::from_slice::<PersistedFile>(&bytes) {
            Ok(parsed) if parsed.version == SCHEMA_VERSION => Ok(parsed.tasks),
            Ok(parsed) => {
                self.quarantine(&format!("unsupported schema version {}", parsed.version))
                    .await?;
                Ok(Vec::new())
            }
            Err(e) => {
                self.quarantine(&format!("serde: {e}")).await?;
                Ok(Vec::new())
            }
        }
    }

    async fn save_all(&self, tasks: &[PersistedTask]) -> Result<(), PersistError> {
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            tokio::fs::create_dir_all(parent).await?;
        }
        let file = PersistedFile {
            version: SCHEMA_VERSION,
            tasks: tasks.to_vec(),
        };
        let bytes = serde_json::to_vec_pretty(&file)?;
        let tmp = self.tmp_path();
        let mut handle = tokio::fs::File::create(&tmp).await?;
        handle.write_all(&bytes).await?;
        handle.sync_all().await?;
        drop(handle);
        tokio::fs::rename(&tmp, &self.path).await?;
        Ok(())
    }
}
