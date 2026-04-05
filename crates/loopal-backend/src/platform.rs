//! Platform-specific helpers and directory listing.

use std::path::Path;

use loopal_error::ToolIoError;
use loopal_tool_api::backend_types::{LsEntry, LsResult};

/// Extract Unix permission bits from file metadata.
#[cfg(unix)]
pub fn extract_permissions(meta: &std::fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    Some(meta.permissions().mode())
}

#[cfg(not(unix))]
pub fn extract_permissions(_meta: &std::fs::Metadata) -> Option<u32> {
    None
}

/// List a directory's contents sorted by name.
pub async fn list_directory(resolved: &Path) -> Result<LsResult, ToolIoError> {
    let mut rd = tokio::fs::read_dir(resolved).await?;
    let mut entries = Vec::new();
    while let Some(entry) = rd.next_entry().await? {
        let meta = entry.metadata().await?;
        let ft = entry.file_type().await?;
        let modified = meta.modified().ok().and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs())
        });
        entries.push(LsEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            is_dir: ft.is_dir(),
            is_symlink: ft.is_symlink(),
            size: meta.len(),
            modified,
            permissions: extract_permissions(&meta),
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(LsResult { entries })
}
