//! `LocalBackend` — production `Backend` for local filesystem + OS sandbox.
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use loopal_config::ResolvedPolicy;
use loopal_error::ToolIoError;
use loopal_tool_api::Backend;
use loopal_tool_api::backend_types::{
    EditResult, ExecResult, FetchResult, FileInfo, GlobOptions, GlobSearchResult, GrepOptions,
    GrepSearchResult, LsEntry, LsResult, ReadResult, WriteResult,
};

use crate::limits::ResourceLimits;
use crate::{fs, net, path, search, shell, shell_stream};

/// Production backend: local disk I/O with path checking, size limits,
/// atomic writes, OS-level sandbox wrapping, and resource budgets.
pub struct LocalBackend {
    cwd: PathBuf,
    policy: Option<ResolvedPolicy>,
    limits: ResourceLimits,
}

impl LocalBackend {
    pub fn new(cwd: PathBuf, policy: Option<ResolvedPolicy>, limits: ResourceLimits) -> Arc<Self> {
        // Canonicalize cwd to resolve symlinks (e.g. macOS /tmp → /private/tmp).
        // On Windows, strip \\?\ prefix that canonicalize() adds.
        let cwd = path::strip_win_prefix(cwd.canonicalize().unwrap_or(cwd));
        Arc::new(Self {
            cwd,
            policy,
            limits,
        })
    }
}

#[async_trait]
impl Backend for LocalBackend {
    async fn read(&self, p: &str, offset: usize, limit: usize) -> Result<ReadResult, ToolIoError> {
        let resolved = path::resolve(&self.cwd, p, false, self.policy.as_ref())?;
        fs::read_file(&resolved, offset, limit, &self.limits).await
    }

    async fn write(&self, p: &str, content: &str) -> Result<WriteResult, ToolIoError> {
        let resolved = path::resolve(&self.cwd, p, true, self.policy.as_ref())?;
        fs::write_file(&resolved, content).await
    }

    async fn edit(
        &self,
        p: &str,
        old: &str,
        new: &str,
        replace_all: bool,
    ) -> Result<EditResult, ToolIoError> {
        let resolved = path::resolve(&self.cwd, p, true, self.policy.as_ref())?;
        fs::edit_file(&resolved, old, new, replace_all).await
    }

    async fn remove(&self, p: &str) -> Result<(), ToolIoError> {
        let resolved = path::resolve(&self.cwd, p, true, self.policy.as_ref())?;
        let meta = tokio::fs::metadata(&resolved).await?;
        if meta.is_dir() {
            tokio::fs::remove_dir_all(&resolved).await?;
        } else {
            tokio::fs::remove_file(&resolved).await?;
        }
        Ok(())
    }

    async fn create_dir_all(&self, p: &str) -> Result<(), ToolIoError> {
        let resolved = path::resolve(&self.cwd, p, true, self.policy.as_ref())?;
        tokio::fs::create_dir_all(&resolved).await?;
        Ok(())
    }

    async fn copy(&self, from: &str, to: &str) -> Result<(), ToolIoError> {
        let src = path::resolve(&self.cwd, from, false, self.policy.as_ref())?;
        let dst = path::resolve(&self.cwd, to, true, self.policy.as_ref())?;
        tokio::fs::copy(&src, &dst).await?;
        Ok(())
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), ToolIoError> {
        let src = path::resolve(&self.cwd, from, true, self.policy.as_ref())?;
        let dst = path::resolve(&self.cwd, to, true, self.policy.as_ref())?;
        tokio::fs::rename(&src, &dst).await?;
        Ok(())
    }

    async fn file_info(&self, p: &str) -> Result<FileInfo, ToolIoError> {
        let resolved = path::resolve(&self.cwd, p, false, self.policy.as_ref())?;
        fs::get_file_info(&resolved).await
    }

    async fn ls(&self, p: &str) -> Result<LsResult, ToolIoError> {
        let resolved = path::resolve(&self.cwd, p, false, self.policy.as_ref())?;
        let mut rd = tokio::fs::read_dir(&resolved).await?;
        let mut entries = Vec::new();
        while let Some(entry) = rd.next_entry().await? {
            let meta = entry.metadata().await?;
            let ft = entry.file_type().await?;
            let modified = meta.modified().ok().and_then(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_secs())
            });
            let permissions = extract_permissions(&meta);
            entries.push(LsEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                is_dir: ft.is_dir(),
                is_symlink: ft.is_symlink(),
                size: meta.len(),
                modified,
                permissions,
            });
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(LsResult { entries })
    }

    async fn glob(&self, opts: &GlobOptions) -> Result<GlobSearchResult, ToolIoError> {
        let opts = opts.clone();
        let cwd = self.cwd.clone();
        let limits = self.limits.clone();
        tokio::task::spawn_blocking(move || search::glob_search(&opts, &cwd, &limits))
            .await
            .map_err(|e| ToolIoError::Other(e.to_string()))?
    }

    async fn grep(&self, opts: &GrepOptions) -> Result<GrepSearchResult, ToolIoError> {
        let opts = opts.clone();
        let cwd = self.cwd.clone();
        let limits = self.limits.clone();
        tokio::task::spawn_blocking(move || search::grep_search(&opts, &cwd, &limits))
            .await
            .map_err(|e| ToolIoError::Other(e.to_string()))?
    }

    fn resolve_path(&self, raw: &str, is_write: bool) -> Result<PathBuf, ToolIoError> {
        path::resolve(&self.cwd, raw, is_write, self.policy.as_ref())
    }

    async fn read_raw(&self, p: &str) -> Result<String, ToolIoError> {
        let resolved = path::resolve(&self.cwd, p, false, self.policy.as_ref())?;
        fs::read_raw_file(&resolved, &self.limits).await
    }

    fn cwd(&self) -> &Path {
        &self.cwd
    }

    async fn exec(&self, command: &str, timeout_ms: u64) -> Result<ExecResult, ToolIoError> {
        shell::exec_command(
            &self.cwd,
            self.policy.as_ref(),
            command,
            timeout_ms,
            &self.limits,
        )
        .await
    }

    async fn exec_streaming(
        &self,
        command: &str,
        timeout_ms: u64,
        tail: Arc<loopal_tool_api::OutputTail>,
    ) -> Result<ExecResult, ToolIoError> {
        shell_stream::exec_command_streaming(
            &self.cwd,
            self.policy.as_ref(),
            command,
            timeout_ms,
            &self.limits,
            tail,
        )
        .await
    }

    async fn exec_background(&self, command: &str, desc: &str) -> Result<String, ToolIoError> {
        shell::exec_background(&self.cwd, self.policy.as_ref(), command, desc).await
    }

    async fn fetch(&self, url: &str) -> Result<FetchResult, ToolIoError> {
        net::fetch_url(url, self.policy.as_ref(), &self.limits).await
    }
}

#[cfg(unix)]
fn extract_permissions(meta: &std::fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    Some(meta.permissions().mode())
}

#[cfg(not(unix))]
fn extract_permissions(_meta: &std::fs::Metadata) -> Option<u32> {
    None
}
