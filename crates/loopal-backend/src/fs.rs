use std::path::Path;

use loopal_error::ToolIoError;
use loopal_tool_api::backend_types::{FileInfo, ReadResult};

use crate::limits::ResourceLimits;

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

pub use crate::fs_write::write_file;

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

fn is_binary(data: &[u8]) -> bool {
    let check_len = data.len().min(8192);
    data[..check_len].contains(&0)
}

pub async fn remove_path(path: &Path) -> Result<(), ToolIoError> {
    let meta = tokio::fs::metadata(path).await?;
    if meta.is_dir() {
        tokio::fs::remove_dir_all(path).await?;
    } else {
        tokio::fs::remove_file(path).await?;
    }
    Ok(())
}

pub async fn peek_bytes(path: &Path, n: usize) -> Result<Vec<u8>, ToolIoError> {
    use tokio::io::AsyncReadExt;
    let mut f = tokio::fs::File::open(path).await?;
    let mut buf = vec![0u8; n];
    let read = f.read(&mut buf).await?;
    buf.truncate(read);
    Ok(buf)
}
