use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use loopal_error::{ProcessHandle, ToolIoError};
use loopal_tool_api::backend_types::{
    EnvOverride, ExecResult, FetchResult, FileInfo, GlobOptions, GlobSearchResult, GrepOptions,
    GrepSearchResult, ImageResult, LsResult, ReadResult, WriteResult,
};
use loopal_tool_api::{
    Backend, BatchOp, BatchOutcome, ExecOutcome, OutputTail, ProcessOutputSanitizer, ResolvedPath,
};

#[derive(Clone, Copy)]
pub enum FailurePoint {
    None,
    ResolveDirectory,
    CreateDirectory,
    ResolveFile,
    WriteFile,
}

pub struct FailureBackend {
    cwd: ResolvedPath,
    failure: FailurePoint,
    resolve_calls: AtomicUsize,
}

impl FailureBackend {
    pub fn new(failure: FailurePoint) -> Arc<Self> {
        let cwd = std::env::temp_dir().join(format!(
            "loopal-fetch-failure-backend-{}",
            std::process::id()
        ));
        Arc::new(Self {
            cwd: ResolvedPath::from_backend_resolved(cwd),
            failure,
            resolve_calls: AtomicUsize::new(0),
        })
    }

    fn unexpected<T>() -> T {
        panic!("unexpected backend operation")
    }
}

#[async_trait]
impl Backend for FailureBackend {
    async fn read(&self, _: &ResolvedPath, _: usize, _: usize) -> Result<ReadResult, ToolIoError> {
        Self::unexpected()
    }

    async fn write(&self, _: &ResolvedPath, content: &str) -> Result<WriteResult, ToolIoError> {
        if matches!(self.failure, FailurePoint::WriteFile) {
            return Err(ToolIoError::Other("write failed".into()));
        }
        Ok(WriteResult {
            bytes_written: content.len(),
        })
    }

    async fn remove(&self, _: &ResolvedPath) -> Result<(), ToolIoError> {
        Self::unexpected()
    }

    async fn create_dir_all(&self, _: &ResolvedPath) -> Result<(), ToolIoError> {
        if matches!(self.failure, FailurePoint::CreateDirectory) {
            return Err(ToolIoError::Other("create failed".into()));
        }
        Ok(())
    }

    async fn copy(&self, _: &ResolvedPath, _: &ResolvedPath) -> Result<(), ToolIoError> {
        Self::unexpected()
    }

    async fn rename(&self, _: &ResolvedPath, _: &ResolvedPath) -> Result<(), ToolIoError> {
        Self::unexpected()
    }

    async fn file_info(&self, _: &ResolvedPath) -> Result<FileInfo, ToolIoError> {
        Self::unexpected()
    }

    async fn ls(&self, _: &ResolvedPath) -> Result<LsResult, ToolIoError> {
        Self::unexpected()
    }

    async fn glob(&self, _: &GlobOptions) -> Result<GlobSearchResult, ToolIoError> {
        Self::unexpected()
    }

    async fn grep(&self, _: &GrepOptions) -> Result<GrepSearchResult, ToolIoError> {
        Self::unexpected()
    }

    fn resolve_path(&self, raw: &str, _: bool) -> Result<ResolvedPath, ToolIoError> {
        let call = self.resolve_calls.fetch_add(1, Ordering::Relaxed);
        if matches!(self.failure, FailurePoint::ResolveDirectory) && call == 0 {
            return Err(ToolIoError::Other("directory resolve failed".into()));
        }
        if matches!(self.failure, FailurePoint::ResolveFile) && call == 1 {
            return Err(ToolIoError::Other("file resolve failed".into()));
        }
        Ok(ResolvedPath::from_backend_resolved(raw.into()))
    }

    async fn read_raw(&self, _: &ResolvedPath) -> Result<String, ToolIoError> {
        Self::unexpected()
    }

    async fn read_image(&self, _: &ResolvedPath) -> Result<ImageResult, ToolIoError> {
        Self::unexpected()
    }

    fn cwd(&self) -> &ResolvedPath {
        &self.cwd
    }

    async fn exec(&self, _: &str, _: Duration, _: &EnvOverride) -> Result<ExecResult, ToolIoError> {
        Self::unexpected()
    }

    async fn exec_guarded(
        &self,
        _: &str,
        _: Duration,
        _: &EnvOverride,
        _: Option<Arc<dyn ProcessOutputSanitizer>>,
    ) -> Result<ExecResult, ToolIoError> {
        Self::unexpected()
    }

    async fn exec_streaming_guarded(
        &self,
        _: &str,
        _: Duration,
        _: &EnvOverride,
        _: Arc<OutputTail>,
        _: Option<Arc<dyn ProcessOutputSanitizer>>,
    ) -> Result<ExecOutcome, ToolIoError> {
        Self::unexpected()
    }

    async fn exec_background(
        &self,
        _: &str,
        _: &EnvOverride,
    ) -> Result<ProcessHandle, ToolIoError> {
        Self::unexpected()
    }

    async fn exec_background_guarded(
        &self,
        _: &str,
        _: &EnvOverride,
        _: Option<Arc<dyn ProcessOutputSanitizer>>,
    ) -> Result<ProcessHandle, ToolIoError> {
        Self::unexpected()
    }

    async fn fetch(&self, _: &str) -> Result<FetchResult, ToolIoError> {
        Ok(FetchResult {
            body: "payload".into(),
            content_type: Some("text/plain".into()),
            status: 200,
            final_url: Some("https://redirect.example/final".into()),
        })
    }

    async fn apply_batch(&self, _: Vec<BatchOp>) -> Result<BatchOutcome, ToolIoError> {
        Self::unexpected()
    }
}
