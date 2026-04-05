//! `LocalBackend` — production `Backend` for local filesystem + OS sandbox.
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_config::ResolvedPolicy;
use loopal_error::{ProcessHandle, ToolIoError};
use loopal_tool_api::backend_types::{
    EditResult, ExecResult, FetchResult, FileInfo, GlobOptions, GlobSearchResult, GrepOptions,
    GrepSearchResult, LsResult, ReadResult, WriteResult,
};
use loopal_tool_api::{Backend, ExecOutcome};

use crate::approved::ApprovedPaths;
use crate::limits::ResourceLimits;
use crate::{fs, net, path, platform, search, shell, shell_stream};

/// Production backend: local disk I/O with path checking, size limits,
/// atomic writes, OS-level sandbox wrapping, and resource budgets.
pub struct LocalBackend {
    cwd: PathBuf,
    policy: Option<ResolvedPolicy>,
    limits: ResourceLimits,
    approved: ApprovedPaths,
}

impl LocalBackend {
    pub fn new(cwd: PathBuf, policy: Option<ResolvedPolicy>, limits: ResourceLimits) -> Arc<Self> {
        let cwd = path::strip_win_prefix(cwd.canonicalize().unwrap_or(cwd));
        Arc::new(Self {
            cwd,
            policy,
            limits,
            approved: ApprovedPaths::new(),
        })
    }

    /// Resolve with sandbox check; falls back to approved-paths on `RequiresApproval`.
    fn resolve_checked(&self, raw: &str, is_write: bool) -> Result<PathBuf, ToolIoError> {
        match path::resolve(&self.cwd, raw, is_write, self.policy.as_ref()) {
            Ok(p) => Ok(p),
            Err(ToolIoError::RequiresApproval(reason)) => {
                let abs = path::to_absolute(&self.cwd, raw);
                if self.approved.contains(&abs) {
                    // Canonicalize for consistency with the Allow path
                    // (path::resolve returns canonical form).
                    Ok(abs.canonicalize().unwrap_or(abs))
                } else {
                    Err(ToolIoError::RequiresApproval(reason))
                }
            }
            Err(e) => Err(e),
        }
    }
}

#[async_trait]
impl Backend for LocalBackend {
    async fn read(&self, p: &str, offset: usize, limit: usize) -> Result<ReadResult, ToolIoError> {
        let resolved = self.resolve_checked(p, false)?;
        fs::read_file(&resolved, offset, limit, &self.limits).await
    }

    async fn write(&self, p: &str, content: &str) -> Result<WriteResult, ToolIoError> {
        fs::write_file(&self.resolve_checked(p, true)?, content).await
    }

    async fn edit(
        &self,
        p: &str,
        old: &str,
        new: &str,
        replace_all: bool,
    ) -> Result<EditResult, ToolIoError> {
        fs::edit_file(&self.resolve_checked(p, true)?, old, new, replace_all).await
    }

    async fn remove(&self, p: &str) -> Result<(), ToolIoError> {
        let resolved = self.resolve_checked(p, true)?;
        let meta = tokio::fs::metadata(&resolved).await?;
        if meta.is_dir() {
            tokio::fs::remove_dir_all(&resolved).await?;
        } else {
            tokio::fs::remove_file(&resolved).await?;
        }
        Ok(())
    }

    async fn create_dir_all(&self, p: &str) -> Result<(), ToolIoError> {
        tokio::fs::create_dir_all(self.resolve_checked(p, true)?).await?;
        Ok(())
    }

    async fn copy(&self, from: &str, to: &str) -> Result<(), ToolIoError> {
        let src = self.resolve_checked(from, false)?;
        let dst = self.resolve_checked(to, true)?;
        tokio::fs::copy(&src, &dst).await?;
        Ok(())
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), ToolIoError> {
        let src = self.resolve_checked(from, true)?;
        let dst = self.resolve_checked(to, true)?;
        tokio::fs::rename(&src, &dst).await?;
        Ok(())
    }

    async fn file_info(&self, p: &str) -> Result<FileInfo, ToolIoError> {
        fs::get_file_info(&self.resolve_checked(p, false)?).await
    }

    async fn ls(&self, p: &str) -> Result<LsResult, ToolIoError> {
        platform::list_directory(&self.resolve_checked(p, false)?).await
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
        self.resolve_checked(raw, is_write)
    }

    async fn read_raw(&self, p: &str) -> Result<String, ToolIoError> {
        fs::read_raw_file(&self.resolve_checked(p, false)?, &self.limits).await
    }

    fn cwd(&self) -> &Path {
        &self.cwd
    }

    async fn exec(&self, command: &str, timeout: Duration) -> Result<ExecResult, ToolIoError> {
        shell::exec_command(
            &self.cwd,
            self.policy.as_ref(),
            command,
            timeout,
            &self.limits,
        )
        .await
    }

    async fn exec_streaming(
        &self,
        command: &str,
        timeout: Duration,
        tail: Arc<loopal_tool_api::OutputTail>,
    ) -> Result<ExecOutcome, ToolIoError> {
        shell_stream::exec_command_streaming(
            &self.cwd,
            self.policy.as_ref(),
            command,
            timeout,
            &self.limits,
            tail,
        )
        .await
    }

    async fn exec_background(&self, command: &str) -> Result<ProcessHandle, ToolIoError> {
        let data = shell::exec_background(&self.cwd, self.policy.as_ref(), command).await?;
        Ok(ProcessHandle(Box::new(data)))
    }

    async fn fetch(&self, url: &str) -> Result<FetchResult, ToolIoError> {
        net::fetch_url(url, self.policy.as_ref(), &self.limits).await
    }

    fn approve_path(&self, p: &Path) {
        self.approved.insert(p.to_path_buf());
    }

    fn check_sandbox_path(&self, raw: &str, is_write: bool) -> Option<String> {
        let abs = path::to_absolute(&self.cwd, raw);
        if self.approved.contains(&abs) {
            return None;
        }
        path::check_requires_approval(&self.cwd, raw, is_write, self.policy.as_ref())
    }
}
