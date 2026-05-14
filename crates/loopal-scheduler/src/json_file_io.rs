use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

use crate::persistence::PersistError;

/// Read all bytes from `path`. A missing file yields `Ok(Vec::new())`.
pub async fn read_or_empty(path: &Path) -> Result<Vec<u8>, PersistError> {
    match tokio::fs::read(path).await {
        Ok(b) => Ok(b),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(PersistError::Io(e)),
    }
}

/// Atomically replace `path` with `bytes` via `<name>.tmp` → `sync_all`
/// → `rename`. POSIX guarantees rename is atomic within a filesystem;
/// readers see either the previous or the new contents, never partial.
pub async fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), PersistError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = sibling_with_extension(path, ".tmp");
    let mut handle = tokio::fs::File::create(&tmp).await?;
    handle.write_all(bytes).await?;
    handle.sync_all().await?;
    drop(handle);
    tokio::fs::rename(&tmp, path).await?;
    Ok(())
}

/// Move a corrupt or schema-incompatible file aside to `<path>.bad-<ts>`.
/// On rename failure the original is preserved — callers must refuse
/// further writes to avoid clobbering data they cannot interpret.
pub async fn quarantine_path(path: &Path, reason: &str) -> Result<(), PersistError> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let dest = sibling_with_extension(path, &format!(".bad-{ts}"));
    match tokio::fs::rename(path, &dest).await {
        Ok(()) => {
            tracing::warn!(
                from = %path.display(),
                to = %dest.display(),
                reason,
                "quarantined corrupt durable cron file"
            );
            Ok(())
        }
        Err(e) => {
            tracing::error!(
                path = %path.display(),
                error = %e,
                reason,
                "failed to quarantine corrupt durable cron file — scheduler will refuse to persist until resolved"
            );
            Err(PersistError::Io(e))
        }
    }
}

fn sibling_with_extension(path: &Path, suffix: &str) -> PathBuf {
    let mut tmp = path.to_path_buf();
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
