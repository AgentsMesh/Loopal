use std::sync::Arc;
use std::time::Duration;

use loopal_error::{ProcessHandle, ToolIoError};
use loopal_tool_api::{EnvOverride, ExecOutcome, ExecResult, OutputTail, ProcessOutputSanitizer};

use crate::local::LocalBackend;
use crate::shell_spawn::CapturePolicy;
use crate::{shell, shell_stream};

impl LocalBackend {
    pub(crate) async fn exec_guarded_inner(
        &self,
        command: &str,
        timeout: Duration,
        env: &EnvOverride,
        sanitizer: Option<Arc<dyn ProcessOutputSanitizer>>,
    ) -> Result<ExecResult, ToolIoError> {
        shell::exec_command_guarded(
            self.cwd.as_path(),
            self.policy.as_ref(),
            command,
            env,
            timeout,
            &self.session_id,
            sanitizer,
        )
        .await
    }

    pub(crate) async fn exec_streaming_guarded_inner(
        &self,
        command: &str,
        timeout: Duration,
        env: &EnvOverride,
        tail: Arc<OutputTail>,
        sanitizer: Option<Arc<dyn ProcessOutputSanitizer>>,
    ) -> Result<ExecOutcome, ToolIoError> {
        shell_stream::exec_command_streaming_guarded(
            self.cwd.as_path(),
            self.policy.as_ref(),
            command,
            env,
            timeout,
            tail,
            CapturePolicy::new(&self.session_id, sanitizer),
        )
        .await
    }

    pub(crate) async fn exec_background_guarded_inner(
        &self,
        command: &str,
        env: &EnvOverride,
        sanitizer: Option<Arc<dyn ProcessOutputSanitizer>>,
    ) -> Result<ProcessHandle, ToolIoError> {
        let data = shell::exec_background_guarded(
            self.cwd.as_path(),
            self.policy.as_ref(),
            command,
            env,
            &self.session_id,
            sanitizer,
        )
        .await?;
        Ok(ProcessHandle(Box::new(data)))
    }
}
