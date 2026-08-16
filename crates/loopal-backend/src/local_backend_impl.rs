use std::sync::Arc;
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

use crate::local::LocalBackend;
use crate::{batch, fs, image, net, path, platform, search};

#[async_trait]
impl Backend for LocalBackend {
    async fn read(
        &self,
        p: &ResolvedPath,
        offset: usize,
        limit: usize,
    ) -> Result<ReadResult, ToolIoError> {
        fs::read_file(p.as_path(), offset, limit, &self.limits).await
    }

    async fn write(&self, p: &ResolvedPath, content: &str) -> Result<WriteResult, ToolIoError> {
        fs::write_file(p.as_path(), content).await
    }

    async fn remove(&self, p: &ResolvedPath) -> Result<(), ToolIoError> {
        fs::remove_path(p.as_path()).await
    }

    async fn create_dir_all(&self, p: &ResolvedPath) -> Result<(), ToolIoError> {
        tokio::fs::create_dir_all(p.as_path()).await?;
        Ok(())
    }

    async fn copy(&self, from: &ResolvedPath, to: &ResolvedPath) -> Result<(), ToolIoError> {
        tokio::fs::copy(from.as_path(), to.as_path()).await?;
        Ok(())
    }

    async fn rename(&self, from: &ResolvedPath, to: &ResolvedPath) -> Result<(), ToolIoError> {
        tokio::fs::rename(from.as_path(), to.as_path()).await?;
        Ok(())
    }

    async fn file_info(&self, p: &ResolvedPath) -> Result<FileInfo, ToolIoError> {
        fs::get_file_info(p.as_path()).await
    }

    async fn ls(&self, p: &ResolvedPath) -> Result<LsResult, ToolIoError> {
        platform::list_directory(p.as_path()).await
    }

    async fn glob(&self, opts: &GlobOptions) -> Result<GlobSearchResult, ToolIoError> {
        search::glob_search_async(
            opts.clone(),
            self.cwd.as_path().to_path_buf(),
            self.limits.clone(),
        )
        .await
    }

    async fn grep(&self, opts: &GrepOptions) -> Result<GrepSearchResult, ToolIoError> {
        search::grep_search_async(
            opts.clone(),
            self.cwd.as_path().to_path_buf(),
            self.limits.clone(),
        )
        .await
    }

    fn resolve_path(&self, raw: &str, is_write: bool) -> Result<ResolvedPath, ToolIoError> {
        self.resolve_checked(raw, is_write)
            .map(ResolvedPath::from_backend_resolved)
    }

    async fn apply_batch(&self, ops: Vec<BatchOp>) -> Result<BatchOutcome, ToolIoError> {
        Ok(batch::apply_batch(ops).await)
    }

    async fn read_raw(&self, p: &ResolvedPath) -> Result<String, ToolIoError> {
        fs::read_raw_file(p.as_path(), &self.limits).await
    }

    async fn read_image(&self, p: &ResolvedPath) -> Result<ImageResult, ToolIoError> {
        image::read_image_resolved(p.as_path(), &self.limits).await
    }

    async fn peek_bytes(&self, p: &ResolvedPath, n: usize) -> Result<Vec<u8>, ToolIoError> {
        fs::peek_bytes(p.as_path(), n).await
    }

    fn cwd(&self) -> &ResolvedPath {
        &self.cwd
    }

    async fn exec(
        &self,
        command: &str,
        timeout: Duration,
        env: &EnvOverride,
    ) -> Result<ExecResult, ToolIoError> {
        self.exec_guarded_inner(command, timeout, env, None).await
    }

    async fn exec_guarded(
        &self,
        command: &str,
        timeout: Duration,
        env: &EnvOverride,
        sanitizer: Option<Arc<dyn ProcessOutputSanitizer>>,
    ) -> Result<ExecResult, ToolIoError> {
        self.exec_guarded_inner(command, timeout, env, sanitizer)
            .await
    }

    async fn exec_streaming(
        &self,
        command: &str,
        timeout: Duration,
        env: &EnvOverride,
        tail: Arc<OutputTail>,
    ) -> Result<ExecOutcome, ToolIoError> {
        self.exec_streaming_guarded_inner(command, timeout, env, tail, None)
            .await
    }

    async fn exec_streaming_guarded(
        &self,
        command: &str,
        timeout: Duration,
        env: &EnvOverride,
        tail: Arc<OutputTail>,
        sanitizer: Option<Arc<dyn ProcessOutputSanitizer>>,
    ) -> Result<ExecOutcome, ToolIoError> {
        self.exec_streaming_guarded_inner(command, timeout, env, tail, sanitizer)
            .await
    }

    async fn exec_background(
        &self,
        command: &str,
        env: &EnvOverride,
    ) -> Result<ProcessHandle, ToolIoError> {
        self.exec_background_guarded_inner(command, env, None).await
    }

    async fn exec_background_guarded(
        &self,
        command: &str,
        env: &EnvOverride,
        sanitizer: Option<Arc<dyn ProcessOutputSanitizer>>,
    ) -> Result<ProcessHandle, ToolIoError> {
        self.exec_background_guarded_inner(command, env, sanitizer)
            .await
    }

    async fn fetch(&self, url: &str) -> Result<FetchResult, ToolIoError> {
        net::fetch_url(url, self.policy.as_ref(), &self.limits).await
    }

    fn approve_path(&self, p: &ResolvedPath) {
        self.approved.insert(p.as_path().to_path_buf());
    }

    fn check_sandbox_path(&self, raw: &str, is_write: bool) -> Option<String> {
        let abs = path::to_absolute(self.cwd.as_path(), raw);
        if self.approved.contains(&abs) {
            return None;
        }
        path::check_requires_approval(self.cwd.as_path(), raw, is_write, self.policy.as_ref())
    }
}
