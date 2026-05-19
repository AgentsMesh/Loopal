use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use loopal_error::StorageError;
use loopal_tool_invocation::ImageMime;
use sha2::{Digest, Sha256};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

#[async_trait]
pub trait ResourceStore: Send + Sync {
    async fn write(
        &self,
        session_id: &str,
        media_type: &str,
        bytes: &[u8],
    ) -> Result<String, StorageError>;

    async fn read(&self, session_id: &str, id: &str) -> Result<Vec<u8>, StorageError>;

    async fn delete_session(&self, session_id: &str) -> Result<(), StorageError>;
}

pub struct FileResourceStore {
    base_dir: PathBuf,
}

impl FileResourceStore {
    pub fn new() -> Result<Arc<Self>, StorageError> {
        let base_dir =
            loopal_config::global_config_dir().map_err(|_| StorageError::HomeDirNotFound)?;
        Ok(Arc::new(Self { base_dir }))
    }

    pub fn with_base_dir(base_dir: PathBuf) -> Arc<Self> {
        Arc::new(Self { base_dir })
    }

    fn resources_dir(&self, session_id: &str) -> PathBuf {
        self.base_dir
            .join("sessions")
            .join(session_id)
            .join("resources")
    }

    fn file_path(&self, session_id: &str, id: &str) -> PathBuf {
        self.resources_dir(session_id).join(id)
    }
}

fn validate_media_type(mime: &str) -> Result<(), StorageError> {
    ImageMime::from_mime_str(mime)
        .map(|_| ())
        .ok_or_else(|| StorageError::Serialization(format!("unsupported media type: {mime}")))
}

pub(crate) fn hash_content(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let mut s = String::with_capacity(32);
    for b in &hash[..16] {
        use std::fmt::Write;
        write!(&mut s, "{b:02x}").unwrap();
    }
    s
}

#[async_trait]
impl ResourceStore for FileResourceStore {
    async fn write(
        &self,
        session_id: &str,
        media_type: &str,
        bytes: &[u8],
    ) -> Result<String, StorageError> {
        validate_media_type(media_type)?;
        let id = hash_content(bytes);
        let dir = self.resources_dir(session_id);
        fs::create_dir_all(&dir).await?;
        let path = self.file_path(session_id, &id);
        // reason: content-addressed; if hash collides on disk the bytes are
        // identical, so we can skip the write entirely.
        if fs::metadata(&path).await.is_ok() {
            return Ok(id);
        }
        let tmp = dir.join(format!("{id}.{}.tmp", Uuid::new_v4().simple()));
        let mut file = fs::File::create(&tmp).await?;
        file.write_all(bytes).await?;
        file.sync_all().await?;
        drop(file);
        // reason: rename is atomic and overwrites; if another writer beat us,
        // the target already has identical bytes so overwrite is harmless.
        fs::rename(&tmp, &path).await?;
        Ok(id)
    }

    async fn read(&self, session_id: &str, id: &str) -> Result<Vec<u8>, StorageError> {
        let path = self.file_path(session_id, id);
        let bytes = fs::read(&path).await?;
        Ok(bytes)
    }

    async fn delete_session(&self, session_id: &str) -> Result<(), StorageError> {
        let dir = self.resources_dir(session_id);
        match fs::remove_dir_all(&dir).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
