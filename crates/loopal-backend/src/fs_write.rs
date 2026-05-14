use std::path::Path;

use loopal_error::ToolIoError;
use loopal_tool_api::backend_types::{EditResult, WriteResult};

pub async fn write_file(path: &Path, content: &str) -> Result<WriteResult, ToolIoError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let stem = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp_name = format!(".{stem}.{}.loopal.tmp", std::process::id());
    let tmp_path = path.with_file_name(tmp_name);
    tokio::fs::write(&tmp_path, content).await?;

    // reason: Windows FlushFileBuffers requires GENERIC_WRITE; must open with write access for fsync.
    let f = tokio::fs::OpenOptions::new()
        .write(true)
        .open(&tmp_path)
        .await?;
    f.sync_all().await?;
    drop(f);

    if let Err(e) = tokio::fs::rename(&tmp_path, path).await {
        // reason: Windows rename-over existing file can fail with ACCESS_DENIED (os error 5)
        // when paths use the \\?\ extended prefix. Fall back to remove + rename.
        if cfg!(windows) && e.raw_os_error() == Some(5) && path.exists() {
            tokio::fs::remove_file(path).await?;
            tokio::fs::rename(&tmp_path, path).await?;
        } else {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(e.into());
        }
    }

    Ok(WriteResult {
        bytes_written: content.len(),
    })
}

pub async fn edit_file(
    path: &Path,
    old: &str,
    new: &str,
    replace_all: bool,
) -> Result<EditResult, ToolIoError> {
    let content = tokio::fs::read_to_string(path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ToolIoError::NotFound(format!("{}", path.display()))
        } else {
            ToolIoError::Io(e)
        }
    })?;

    use loopal_edit_core::search_replace::{SearchReplaceResult, search_replace};
    match search_replace(&content, old, new, replace_all) {
        SearchReplaceResult::Ok(new_content) => {
            let count = if replace_all {
                content.matches(old).count()
            } else {
                1
            };
            write_file(path, &new_content).await?;
            Ok(EditResult {
                replacements: count,
            })
        }
        SearchReplaceResult::NotFound => {
            Err(ToolIoError::Other("old_string not found in file".into()))
        }
        SearchReplaceResult::MultipleMatches(n) => Err(ToolIoError::Other(format!(
            "old_string found {n} times — use replace_all or provide more context"
        ))),
    }
}
