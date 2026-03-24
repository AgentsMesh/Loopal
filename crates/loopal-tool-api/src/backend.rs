use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use loopal_error::ToolIoError;

use crate::backend_types::{
    EditResult, ExecResult, FetchResult, FileInfo, GlobOptions, GlobSearchResult, GrepOptions,
    GrepSearchResult, LsResult, ReadResult, WriteResult,
};
use crate::output_tail::OutputTail;

/// Capability-based I/O abstraction injected into tools via `ToolContext`.
///
/// Tool crates call `ctx.backend().<method>()` instead of doing raw
/// `tokio::fs` / `tokio::process` / `reqwest` I/O.  Because tool crates
/// never list those runtime crates in their `Cargo.toml`, the Cargo resolver
/// guarantees at *compile time* that tools cannot bypass this interface.
///
/// The default production implementation is `LocalBackend` (in `loopal-backend`),
/// which adds path checking, size limits, atomic writes, OS-level sandbox
/// wrapping, and resource budgets.
#[async_trait]
pub trait Backend: Send + Sync {
    // --- Filesystem ---

    /// Read file content with offset/limit pagination.
    async fn read(
        &self,
        path: &str,
        offset: usize,
        limit: usize,
    ) -> Result<ReadResult, ToolIoError>;

    /// Write content to a file (atomic: write-tmp → fsync → rename).
    async fn write(&self, path: &str, content: &str) -> Result<WriteResult, ToolIoError>;

    /// Search-and-replace edit on an existing file.
    async fn edit(
        &self,
        path: &str,
        old: &str,
        new: &str,
        replace_all: bool,
    ) -> Result<EditResult, ToolIoError>;

    /// Remove a file or directory.
    async fn remove(&self, path: &str) -> Result<(), ToolIoError>;

    /// Create directory tree (like `mkdir -p`).
    async fn create_dir_all(&self, path: &str) -> Result<(), ToolIoError>;

    /// Copy a file or directory.
    async fn copy(&self, from: &str, to: &str) -> Result<(), ToolIoError>;

    /// Rename / move a file or directory.
    async fn rename(&self, from: &str, to: &str) -> Result<(), ToolIoError>;

    /// Query file metadata.
    async fn file_info(&self, path: &str) -> Result<FileInfo, ToolIoError>;

    /// List directory contents.
    async fn ls(&self, path: &str) -> Result<LsResult, ToolIoError>;

    /// Glob pattern search from an optional base directory.
    async fn glob(&self, opts: &GlobOptions) -> Result<GlobSearchResult, ToolIoError>;

    /// Regex search over file contents with context, multiline, and type filtering.
    async fn grep(&self, opts: &GrepOptions) -> Result<GrepSearchResult, ToolIoError>;

    /// Resolve a user-supplied path to a canonical absolute path.
    /// `is_write` triggers write-permission checks when a sandbox policy is active.
    fn resolve_path(&self, raw: &str, is_write: bool) -> Result<PathBuf, ToolIoError>;

    /// Read raw file content with path checking, size limit, and binary detection.
    /// Unlike `read()`, does NOT add line numbers — returns the original content.
    async fn read_raw(&self, path: &str) -> Result<String, ToolIoError>;

    /// Current working directory of this backend.
    fn cwd(&self) -> &Path;

    // --- Command execution ---

    /// Execute a shell command synchronously (with timeout).
    async fn exec(&self, command: &str, timeout_ms: u64) -> Result<ExecResult, ToolIoError>;

    /// Execute a shell command with streaming output capture.
    ///
    /// Like `exec`, but feeds stdout/stderr lines into `tail` in real time.
    /// Default implementation ignores `tail` and delegates to `exec`.
    async fn exec_streaming(
        &self,
        command: &str,
        timeout_ms: u64,
        _tail: Arc<OutputTail>,
    ) -> Result<ExecResult, ToolIoError> {
        self.exec(command, timeout_ms).await
    }

    /// Spawn a command in the background; returns a task ID.
    async fn exec_background(&self, command: &str, desc: &str) -> Result<String, ToolIoError>;

    // --- Network ---

    /// Fetch content from a URL.
    async fn fetch(&self, url: &str) -> Result<FetchResult, ToolIoError>;
}
