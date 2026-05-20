use loopal_error::ToolIoError;
use loopal_tool_api::{Backend, ResolvedPath};

pub(crate) struct DstResolution {
    /// Backend-resolved (canonicalized, sandboxed) final destination.
    pub resolved: ResolvedPath,
    /// User-input-derived display form (preserves relative-path shape for
    /// friendly messages). Refers to the same logical target as `resolved`
    /// but in a different representation (e.g. `subdir/foo.txt` vs the
    /// canonicalized absolute path).
    pub display: String,
}

pub(crate) async fn resolve_dst(
    backend: &dyn Backend,
    src_raw: &str,
    dst_raw: &str,
) -> Result<DstResolution, ToolIoError> {
    let initial = backend.resolve_path(dst_raw, true)?;

    match backend.file_info(&initial).await {
        Ok(info) if info.is_dir => {
            let name = std::path::Path::new(src_raw)
                .file_name()
                .ok_or_else(|| ToolIoError::Other("source has no file name".into()))?;
            let final_raw = std::path::Path::new(dst_raw)
                .join(name)
                .to_string_lossy()
                .into_owned();
            let resolved = backend.resolve_path(&final_raw, true)?;
            Ok(DstResolution {
                resolved,
                display: final_raw,
            })
        }
        _ => Ok(DstResolution {
            resolved: initial,
            display: dst_raw.to_string(),
        }),
    }
}

pub(crate) async fn ensure_parent_dir(
    backend: &dyn Backend,
    target: &ResolvedPath,
) -> Result<(), ToolIoError> {
    let Some(parent) = target.as_path().parent() else {
        return Ok(());
    };
    let parent_str = parent.to_string_lossy();
    if parent_str.is_empty() {
        return Ok(());
    }
    let parent_resolved = backend.resolve_path(&parent_str, true)?;
    backend.create_dir_all(&parent_resolved).await
}
