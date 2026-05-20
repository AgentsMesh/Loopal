use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_error::{ProcessHandle, ToolIoError};

use crate::backend_types::{
    EnvOverride, ExecResult, FetchResult, FileInfo, GlobOptions, GlobSearchResult, GrepOptions,
    GrepSearchResult, ImageResult, LsResult, ReadResult, WriteResult,
};
use crate::batch::{BatchOp, BatchOutcome};
use crate::output_tail::OutputTail;
use crate::path::ResolvedPath;

pub enum ExecOutcome {
    Completed(ExecResult),
    TimedOut {
        timeout: Duration,
        partial_output: String,
        handle: ProcessHandle,
    },
}

#[async_trait]
pub trait Backend: Send + Sync {
    // --- Filesystem ---

    async fn read(
        &self,
        path: &ResolvedPath,
        offset: usize,
        limit: usize,
    ) -> Result<ReadResult, ToolIoError>;

    async fn write(&self, path: &ResolvedPath, content: &str) -> Result<WriteResult, ToolIoError>;

    async fn remove(&self, path: &ResolvedPath) -> Result<(), ToolIoError>;

    async fn create_dir_all(&self, path: &ResolvedPath) -> Result<(), ToolIoError>;

    async fn copy(&self, from: &ResolvedPath, to: &ResolvedPath) -> Result<(), ToolIoError>;

    async fn rename(&self, from: &ResolvedPath, to: &ResolvedPath) -> Result<(), ToolIoError>;

    async fn file_info(&self, path: &ResolvedPath) -> Result<FileInfo, ToolIoError>;

    async fn ls(&self, path: &ResolvedPath) -> Result<LsResult, ToolIoError>;

    async fn glob(&self, opts: &GlobOptions) -> Result<GlobSearchResult, ToolIoError>;

    async fn grep(&self, opts: &GrepOptions) -> Result<GrepSearchResult, ToolIoError>;

    fn resolve_path(&self, raw: &str, is_write: bool) -> Result<ResolvedPath, ToolIoError>;

    async fn read_raw(&self, path: &ResolvedPath) -> Result<String, ToolIoError>;

    async fn read_image(&self, path: &ResolvedPath) -> Result<ImageResult, ToolIoError>;

    async fn peek_bytes(&self, _path: &ResolvedPath, _n: usize) -> Result<Vec<u8>, ToolIoError> {
        Ok(Vec::new())
    }

    fn cwd(&self) -> &ResolvedPath;

    // --- Command execution ---

    async fn exec(
        &self,
        command: &str,
        timeout: Duration,
        env: &EnvOverride,
    ) -> Result<ExecResult, ToolIoError>;

    async fn exec_streaming(
        &self,
        command: &str,
        timeout: Duration,
        env: &EnvOverride,
        _tail: Arc<OutputTail>,
    ) -> Result<ExecOutcome, ToolIoError> {
        self.exec(command, timeout, env)
            .await
            .map(ExecOutcome::Completed)
    }

    async fn exec_background(
        &self,
        command: &str,
        env: &EnvOverride,
    ) -> Result<ProcessHandle, ToolIoError>;

    // --- Network ---

    async fn fetch(&self, url: &str) -> Result<FetchResult, ToolIoError>;

    // --- Batch ---

    async fn apply_batch(&self, ops: Vec<BatchOp>) -> Result<BatchOutcome, ToolIoError>;

    // --- Sandbox approval ---

    /// Record a path as approved for this session (sandbox RequiresApproval flow).
    fn approve_path(&self, _path: &ResolvedPath) {}

    /// Pre-check whether a raw path would require sandbox approval (pre-resolve).
    fn check_sandbox_path(&self, _raw: &str, _is_write: bool) -> Option<String> {
        None
    }
}
