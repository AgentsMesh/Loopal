use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_error::{ProcessHandle, ToolIoError};
use loopal_tool_api::backend_types::{
    EnvOverride, ExecResult, FetchResult, FileInfo, GlobOptions, GlobSearchResult, GrepOptions,
    GrepSearchResult, ImageResult, LsResult, ReadResult, WriteResult,
};
use loopal_tool_api::{Backend, BatchOp, BatchOutcome, ExecOutcome, OutputTail, ResolvedPath};

use crate::local::LocalBackend;
use crate::{batch, fs, image, net, path, platform, search, shell, shell_stream};

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
        shell::exec_command(
            self.cwd.as_path(),
            self.policy.as_ref(),
            command,
            env,
            timeout,
            &self.session_id,
        )
        .await
    }

    async fn exec_streaming(
        &self,
        command: &str,
        timeout: Duration,
        env: &EnvOverride,
        tail: Arc<OutputTail>,
    ) -> Result<ExecOutcome, ToolIoError> {
        shell_stream::exec_command_streaming(
            self.cwd.as_path(),
            self.policy.as_ref(),
            command,
            env,
            timeout,
            tail,
            &self.session_id,
        )
        .await
    }

    async fn exec_background(
        &self,
        command: &str,
        env: &EnvOverride,
    ) -> Result<ProcessHandle, ToolIoError> {
        let data = shell::exec_background(
            self.cwd.as_path(),
            self.policy.as_ref(),
            command,
            env,
            &self.session_id,
        )
        .await?;
        Ok(ProcessHandle(Box::new(data)))
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
