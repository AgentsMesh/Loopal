//! File system operations with built-in safety (size limits, binary detection, atomic writes).

use std::path::Path;

use loopal_error::ToolIoError;
use loopal_tool_api::backend_types::{EditResult, FileInfo, ReadResult, WriteResult};

use crate::limits::ResourceLimits;

/// Read a text file with offset/limit pagination and line numbering.
pub async fn read_file(
    path: &Path,
    offset: usize,
    limit: usize,
    limits: &ResourceLimits,
) -> Result<ReadResult, ToolIoError> {
    let meta = tokio::fs::metadata(path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ToolIoError::NotFound(format!("{}", path.display()))
        } else {
            ToolIoError::Io(e)
        }
    })?;

    if meta.len() > limits.max_file_read_bytes {
        return Err(ToolIoError::TooLarge {
            path: path.display().to_string(),
            size: meta.len(),
            limit: limits.max_file_read_bytes,
        });
    }

    let bytes = tokio::fs::read(path).await?;

    if is_binary(&bytes) {
        return Err(ToolIoError::BinaryFile(path.display().to_string()));
    }

    let content = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();
    let start = offset.min(total_lines);
    let end = (start + limit).min(total_lines);

    let mut result = String::new();
    for (i, line) in lines[start..end].iter().enumerate() {
        let line_num = start + i + 1;
        result.push_str(&format!("{line_num:>6}\t{line}\n"));
    }
    Ok(ReadResult {
        content: result,
        total_lines,
    })
}

/// Read raw file content with size check and binary detection (no line numbering).
pub async fn read_raw_file(path: &Path, limits: &ResourceLimits) -> Result<String, ToolIoError> {
    let meta = tokio::fs::metadata(path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ToolIoError::NotFound(format!("{}", path.display()))
        } else {
            ToolIoError::Io(e)
        }
    })?;

    if meta.len() > limits.max_file_read_bytes {
        return Err(ToolIoError::TooLarge {
            path: path.display().to_string(),
            size: meta.len(),
            limit: limits.max_file_read_bytes,
        });
    }

    let bytes = tokio::fs::read(path).await?;

    if is_binary(&bytes) {
        return Err(ToolIoError::BinaryFile(path.display().to_string()));
    }

    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Atomic write: write to tmp → fsync → rename.
///
/// On Windows, rename-over can fail with `\\?\` extended paths, so we
/// fall back to remove + rename when we get ACCESS_DENIED.
pub async fn write_file(path: &Path, content: &str) -> Result<WriteResult, ToolIoError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Use pid in temp name to avoid collision with concurrent processes
    let stem = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp_name = format!(".{stem}.{}.loopal.tmp", std::process::id());
    let tmp_path = path.with_file_name(tmp_name);
    tokio::fs::write(&tmp_path, content).await?;

    // fsync the temp file for durability
    let f = tokio::fs::File::open(&tmp_path).await?;
    f.sync_all().await?;
    drop(f);

    if let Err(e) = tokio::fs::rename(&tmp_path, path).await {
        // Windows: rename-over existing file can fail with ACCESS_DENIED (os error 5)
        // when paths use the \\?\ extended prefix. Fall back to remove + rename.
        if cfg!(windows) && e.raw_os_error() == Some(5) && path.exists() {
            tokio::fs::remove_file(path).await?;
            tokio::fs::rename(&tmp_path, path).await?;
        } else {
            // Clean up temp file on failure
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(e.into());
        }
    }

    Ok(WriteResult {
        bytes_written: content.len(),
    })
}

/// Search-and-replace edit using edit-core's search_replace.
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

/// Query file metadata including binary detection.
pub async fn get_file_info(path: &Path) -> Result<FileInfo, ToolIoError> {
    let meta = tokio::fs::metadata(path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ToolIoError::NotFound(format!("{}", path.display()))
        } else {
            ToolIoError::Io(e)
        }
    })?;

    let is_binary = if meta.is_file() && meta.len() > 0 {
        let mut buf = vec![0u8; 8192.min(meta.len() as usize)];
        if let Ok(mut f) = tokio::fs::File::open(path).await {
            use tokio::io::AsyncReadExt;
            let n = f.read(&mut buf).await.unwrap_or(0);
            is_binary(&buf[..n])
        } else {
            false
        }
    } else {
        false
    };

    let modified = meta.modified().ok().and_then(|t| {
        t.duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs())
    });

    Ok(FileInfo {
        size: meta.len(),
        is_dir: meta.is_dir(),
        is_binary,
        modified,
    })
}

/// Detect binary content by checking for null bytes in the first 8 KB.
fn is_binary(data: &[u8]) -> bool {
    let check_len = data.len().min(8192);
    data[..check_len].contains(&0)
}
