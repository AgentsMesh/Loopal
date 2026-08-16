use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_error::{ProcessHandle, ToolIoError};
use loopal_tool_api::{
    EnvOverride, ExecOutcome, ExecResult, OutputTail, ProcessExecutor, ToolContext,
};

use super::{background, foreground, streaming};

struct StubExecutor;

#[async_trait]
impl ProcessExecutor for StubExecutor {
    async fn exec(
        &self,
        _command: &str,
        _timeout: Duration,
        _env: &EnvOverride,
    ) -> Result<ExecResult, ToolIoError> {
        Ok(result("executor foreground"))
    }

    async fn exec_streaming(
        &self,
        _command: &str,
        _timeout: Duration,
        _env: &EnvOverride,
        tail: Arc<OutputTail>,
    ) -> Result<ExecOutcome, ToolIoError> {
        tail.push_line("executor progress".into());
        Ok(ExecOutcome::Completed(result("executor streaming")))
    }

    async fn exec_background(
        &self,
        _command: &str,
        _env: &EnvOverride,
    ) -> Result<ProcessHandle, ToolIoError> {
        Ok(ProcessHandle(Box::new(String::from("executor background"))))
    }
}

fn result(stdout: &str) -> ExecResult {
    ExecResult {
        stdout: stdout.into(),
        stderr: String::new(),
        stdout_truncated: false,
        stderr_truncated: false,
        exit_code: 0,
        log_path: std::path::PathBuf::new(),
    }
}

fn context() -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
        "process-exec-unit",
    );
    let mut context = ToolContext::new(backend, "process-exec-unit");
    context.process_executor = Some(Arc::new(StubExecutor));
    context
}

#[tokio::test]
async fn injected_executor_owns_foreground_streaming_and_background_routes() {
    let context = context();
    let env = EnvOverride::default();

    let foreground = foreground(&context, "must-not-run", Duration::from_secs(1), &env)
        .await
        .unwrap();
    assert_eq!(foreground.stdout, "executor foreground");

    let tail = Arc::new(OutputTail::new(4));
    let streaming = streaming(
        &context,
        "must-not-run",
        Duration::from_secs(1),
        &env,
        tail.clone(),
    )
    .await
    .unwrap();
    let ExecOutcome::Completed(streaming) = streaming else {
        panic!("stub executor must complete streaming");
    };
    assert_eq!(streaming.stdout, "executor streaming");
    assert!(tail.snapshot().contains("executor progress"));

    let handle = background(&context, "must-not-run", &env).await.unwrap();
    assert_eq!(
        *handle.0.downcast::<String>().unwrap(),
        "executor background"
    );
}
