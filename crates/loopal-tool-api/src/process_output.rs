use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_error::{ProcessHandle, ToolIoError};

use crate::{EnvOverride, ExecOutcome, ExecResult, OutputTail};

pub trait ProcessOutputSanitizer: Send + Sync {
    fn stream(&self) -> Box<dyn ProcessOutputStream>;
    fn guard_text(&self, text: &str) -> String;
}

pub trait ProcessOutputStream: Send {
    fn sanitize(&mut self, chunk: &[u8]) -> Vec<u8>;
    fn finish(&mut self) -> Vec<u8>;
    fn committed_input_bytes(&self) -> usize;
}

#[async_trait]
pub trait ProcessExecutor: Send + Sync {
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
        tail: Arc<OutputTail>,
    ) -> Result<ExecOutcome, ToolIoError>;

    async fn exec_background(
        &self,
        command: &str,
        env: &EnvOverride,
    ) -> Result<ProcessHandle, ToolIoError>;
}
