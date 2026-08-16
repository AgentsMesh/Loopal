use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_error::{ProcessHandle, ToolIoError};
use loopal_tool_api::{
    Backend, EnvOverride, ExecOutcome, ExecResult, OutputTail, ProcessExecutor,
    ProcessOutputSanitizer,
};

pub(crate) struct GuardedProcessExecutor {
    backend: Arc<dyn Backend>,
    sanitizer: Arc<dyn ProcessOutputSanitizer>,
}

impl GuardedProcessExecutor {
    pub(crate) fn new(
        backend: Arc<dyn Backend>,
        sanitizer: Arc<dyn ProcessOutputSanitizer>,
    ) -> Self {
        Self { backend, sanitizer }
    }
}

#[async_trait]
impl ProcessExecutor for GuardedProcessExecutor {
    async fn exec(
        &self,
        command: &str,
        timeout: Duration,
        env: &EnvOverride,
    ) -> Result<ExecResult, ToolIoError> {
        self.backend
            .exec_guarded(command, timeout, env, Some(self.sanitizer.clone()))
            .await
    }

    async fn exec_streaming(
        &self,
        command: &str,
        timeout: Duration,
        env: &EnvOverride,
        tail: Arc<OutputTail>,
    ) -> Result<ExecOutcome, ToolIoError> {
        self.backend
            .exec_streaming_guarded(command, timeout, env, tail, Some(self.sanitizer.clone()))
            .await
    }

    async fn exec_background(
        &self,
        command: &str,
        env: &EnvOverride,
    ) -> Result<ProcessHandle, ToolIoError> {
        self.backend
            .exec_background_guarded(command, env, Some(self.sanitizer.clone()))
            .await
    }
}
