use chrono::{DateTime, SecondsFormat, Utc};
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;

use crate::types::{
    DirectoryEntry, DirectoryListing, FileDocument, WorkspacePathParams, WriteFileParams,
};
use crate::{WorkspaceError, WorkspaceService};

const MAX_FILE_BYTES: usize = 10_000_000;
const MAX_DIRECTORY_ENTRIES: usize = 10_000;

pub(crate) fn version(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

impl WorkspaceService {
    pub async fn read_file(
        &self,
        input: WorkspacePathParams,
    ) -> Result<FileDocument, WorkspaceError> {
        self.require_workspace(&input.workspace_id)?;
        self.read_document(&input.path).await
    }

    pub async fn write_file(&self, input: WriteFileParams) -> Result<FileDocument, WorkspaceError> {
        self.require_workspace(&input.workspace_id)?;
        if input.content.len() > MAX_FILE_BYTES {
            return Err(WorkspaceError::new(
                "file_too_large",
                "content exceeds 10 MB",
            ));
        }
        let _lock = self.write_lock.lock().await;
        let path = self.guard.resolve(&input.path, true)?;
        let current = match tokio::fs::File::open(&path).await {
            Ok(file) => Some(read_open(file, &path).await?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let current_version = current.as_deref().map(version);
        if current_version != input.expected_version {
            return Err(WorkspaceError::new(
                "version_conflict",
                format!(
                    "expected {:?}, current {:?}",
                    input.expected_version, current_version
                ),
            ));
        }
        if current.is_some() && tokio::fs::metadata(&path).await?.permissions().readonly() {
            return Err(WorkspaceError::new("readonly_file", "file is read-only"));
        }
        let relative = self.guard.relative(&path)?;
        let kind = if current.is_some() {
            "changed"
        } else {
            "created"
        };
        let expected = match current.as_deref() {
            Some(bytes) => loopal_backend::fs::ExpectedContent::Bytes(bytes),
            None => loopal_backend::fs::ExpectedContent::Missing,
        };
        let written = loopal_backend::fs::write_file_if_unchanged(&path, &input.content, expected)
            .await
            .map_err(WorkspaceError::io)?;
        if written.is_none() {
            return Err(WorkspaceError::new(
                "version_conflict",
                "file changed while preparing the atomic write",
            ));
        }
        self.publish_file_changed(&relative, kind);
        self.publish_git_changed();
        self.read_document(&input.path).await
    }

    pub async fn list_directory(
        &self,
        input: WorkspacePathParams,
    ) -> Result<DirectoryListing, WorkspaceError> {
        self.require_workspace(&input.workspace_id)?;
        let path = self.guard.resolve(&input.path, false)?;
        let mut reader = tokio::fs::read_dir(&path).await?;
        let mut entries = Vec::new();
        while let Some(entry) = reader.next_entry().await? {
            if entries.len() == MAX_DIRECTORY_ENTRIES {
                return Err(WorkspaceError::new(
                    "response_too_large",
                    "directory contains more than 10000 entries",
                ));
            }
            let metadata = tokio::fs::symlink_metadata(entry.path()).await?;
            let kind = if metadata.file_type().is_symlink() {
                "symlink"
            } else if metadata.is_dir() {
                "directory"
            } else {
                "file"
            };
            let modified_at = metadata.modified().ok().map(|value| {
                DateTime::<Utc>::from(value).to_rfc3339_opts(SecondsFormat::Millis, true)
            });
            entries.push(DirectoryEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: self.guard.relative(&entry.path())?,
                kind,
                size: metadata.len(),
                modified_at,
            });
        }
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(DirectoryListing {
            workspace_id: self.workspace_id.clone(),
            path: self.guard.relative(&path)?,
            entries,
        })
    }

    async fn read_document(&self, raw: &str) -> Result<FileDocument, WorkspaceError> {
        let path = self.guard.resolve(raw, false)?;
        let bytes = read_open(tokio::fs::File::open(&path).await?, &path).await?;
        let file_version = version(&bytes);
        let content = String::from_utf8(bytes)
            .map_err(|_| WorkspaceError::new("binary_file", "file is not UTF-8 text"))?;
        let readonly = tokio::fs::metadata(&path).await?.permissions().readonly();
        Ok(FileDocument {
            workspace_id: self.workspace_id.clone(),
            path: self.guard.relative(&path)?,
            content,
            version: file_version,
            language_id: language_id(&path),
            readonly,
        })
    }
}

async fn read_open(
    file: tokio::fs::File,
    path: &std::path::Path,
) -> Result<Vec<u8>, WorkspaceError> {
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(WorkspaceError::new(
            "file_too_large",
            format!("{} exceeds 10 MB", path.display()),
        ));
    }
    Ok(bytes)
}

fn language_id(path: &std::path::Path) -> String {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
    {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "json" => "json",
        "md" => "markdown",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "css" => "css",
        "html" => "html",
        "sh" | "zsh" | "bash" => "shellscript",
        "py" => "python",
        "go" => "go",
        _ => "plaintext",
    }
    .to_string()
}
