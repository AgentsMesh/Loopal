use std::path::{Path, PathBuf};

use loopal_error::ToolIoError;
use loopal_tool_api::backend_types::WriteResult;
use tokio::io::AsyncReadExt;

pub enum ExpectedContent<'a> {
    Missing,
    Bytes(&'a [u8]),
}

pub async fn write_file(path: &Path, content: &str) -> Result<WriteResult, ToolIoError> {
    let tmp = prepare(path, content).await?;
    commit(&tmp, path).await?;
    Ok(result(content))
}

pub async fn write_file_if_unchanged(
    path: &Path,
    content: &str,
    expected: ExpectedContent<'_>,
) -> Result<Option<WriteResult>, ToolIoError> {
    let tmp = prepare(path, content).await?;
    let matches = match current_matches(path, expected).await {
        Ok(matches) => matches,
        Err(error) => {
            remove_temp(&tmp).await;
            return Err(error);
        }
    };
    if !matches {
        remove_temp(&tmp).await;
        return Ok(None);
    }
    commit(&tmp, path).await?;
    Ok(Some(result(content)))
}

async fn prepare(path: &Path, content: &str) -> Result<PathBuf, ToolIoError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let stem = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp = path.with_file_name(format!(".{stem}.{}.loopal.tmp", uuid::Uuid::new_v4()));
    let prepared = async {
        tokio::fs::write(&tmp, content).await?;
        let file = tokio::fs::OpenOptions::new().write(true).open(&tmp).await?;
        file.sync_all().await
    }
    .await;
    if let Err(error) = prepared {
        remove_temp(&tmp).await;
        return Err(error.into());
    }
    Ok(tmp)
}

async fn current_matches(path: &Path, expected: ExpectedContent<'_>) -> Result<bool, ToolIoError> {
    let file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(matches!(expected, ExpectedContent::Missing));
        }
        Err(error) => return Err(error.into()),
    };
    match expected {
        ExpectedContent::Missing => Ok(false),
        ExpectedContent::Bytes(expected) => {
            let mut current = Vec::with_capacity(expected.len().saturating_add(1));
            file.take(expected.len() as u64 + 1)
                .read_to_end(&mut current)
                .await?;
            Ok(current == expected)
        }
    }
}

async fn commit(tmp: &Path, path: &Path) -> Result<(), ToolIoError> {
    if let Err(error) = tokio::fs::rename(tmp, path).await {
        #[cfg(windows)]
        if tokio::fs::try_exists(path).await.unwrap_or(false) {
            if let Err(error) = crate::fs_replace::replace_existing(tmp, path).await {
                remove_temp(tmp).await;
                return Err(error.into());
            }
            return Ok(());
        }
        remove_temp(tmp).await;
        return Err(error.into());
    }
    Ok(())
}

async fn remove_temp(path: &Path) {
    let _ = tokio::fs::remove_file(path).await;
}

fn result(content: &str) -> WriteResult {
    WriteResult {
        bytes_written: content.len(),
    }
}
