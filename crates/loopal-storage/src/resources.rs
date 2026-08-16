#[path = "resources/file_io.rs"]
mod file_io;
#[cfg(test)]
#[path = "resources/file_io_tests.rs"]
mod file_io_tests;

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

    async fn read_bounded(
        &self,
        session_id: &str,
        id: &str,
        max_bytes: usize,
    ) -> Result<Vec<u8>, StorageError>;

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

fn validate_image_payload(mime: &str, bytes: &[u8]) -> Result<(), StorageError> {
    let declared = ImageMime::from_mime_str(mime)
        .ok_or_else(|| StorageError::Serialization(format!("unsupported media type: {mime}")))?;
    if ImageMime::from_magic(bytes) != Some(declared) {
        return Err(StorageError::ResourceIntegrity);
    }
    Ok(())
}

fn validate_resource_id(id: &str) -> Result<(), StorageError> {
    let valid = id.len() == 32
        && id
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte));
    if valid {
        Ok(())
    } else {
        Err(StorageError::InvalidResourceId)
    }
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
        crate::sessions::validate_path_component("session_id", session_id)?;
        validate_image_payload(media_type, bytes)?;
        let id = hash_content(bytes);
        let dir = self.resources_dir(session_id);
        fs::create_dir_all(&dir).await?;
        let path = self.file_path(session_id, &id);
        if file_io::existing_matches(&path, bytes).await? {
            return Ok(id);
        }
        let tmp = dir.join(format!("{id}.{}.tmp", Uuid::new_v4().simple()));
        let prepared = async {
            let mut file = file_io::private_create_options().open(&tmp).await?;
            file_io::enforce_private_permissions(&file).await?;
            file.write_all(bytes).await?;
            file.sync_all().await?;
            drop(file);
            file_io::replace_file(&tmp, &path).await
        }
        .await;
        if let Err(error) = prepared {
            let _ = fs::remove_file(&tmp).await;
            return Err(error.into());
        }
        Ok(id)
    }

    async fn read_bounded(
        &self,
        session_id: &str,
        id: &str,
        max_bytes: usize,
    ) -> Result<Vec<u8>, StorageError> {
        crate::sessions::validate_path_component("session_id", session_id)?;
        validate_resource_id(id)?;
        let path = self.file_path(session_id, id);
        let bytes = file_io::read_regular_bounded(&path, max_bytes).await?;
        if hash_content(&bytes) != id {
            return Err(StorageError::ResourceIntegrity);
        }
        Ok(bytes)
    }

    async fn delete_session(&self, session_id: &str) -> Result<(), StorageError> {
        crate::sessions::validate_path_component("session_id", session_id)?;
        let dir = self.resources_dir(session_id);
        match fs::remove_dir_all(&dir).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
