use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::time::Duration;

use futures::FutureExt;
use loopal_error::ToolIoError;
use tokio::task::{JoinError, JoinHandle};

pub struct ProcessCaptureTask {
    pub(crate) handle: Option<JoinHandle<Result<(), ToolIoError>>>,
}

impl ProcessCaptureTask {
    pub(crate) fn spawn(
        future: impl Future<Output = Result<(), ToolIoError>> + Send + 'static,
        on_failure: impl Fn() + Send + 'static,
    ) -> Self {
        let handle = tokio::spawn(async move {
            let result = match AssertUnwindSafe(future).catch_unwind().await {
                Ok(result) => result,
                Err(_) => Err(capture_error("task failed")),
            };
            if result.is_err() {
                on_failure();
            }
            result
        });
        Self {
            handle: Some(handle),
        }
    }
}

impl Drop for ProcessCaptureTask {
    fn drop(&mut self) {
        if let Some(handle) = &self.handle {
            handle.abort();
        }
    }
}

pub async fn join(mut task: ProcessCaptureTask) -> Result<(), ToolIoError> {
    let result = task
        .handle
        .as_mut()
        .expect("capture task consumed once")
        .await;
    task.handle.take();
    task_result(result)
}

pub async fn join_bounded(
    mut task: ProcessCaptureTask,
    grace: Duration,
) -> Result<(), ToolIoError> {
    let handle = task.handle.as_mut().expect("capture task consumed once");
    match tokio::time::timeout(grace, handle).await {
        Ok(result) => {
            task.handle.take();
            task_result(result)
        }
        Err(_) => {
            let handle = task.handle.take().expect("capture task present");
            handle.abort();
            let _ = handle.await;
            Err(capture_error("drain timed out"))
        }
    }
}

fn task_result(result: Result<Result<(), ToolIoError>, JoinError>) -> Result<(), ToolIoError> {
    match result {
        Ok(result) => result,
        Err(error) if error.is_cancelled() => Err(capture_error("task cancelled")),
        Err(_) => Err(capture_error("task failed")),
    }
}

fn capture_error(stage: &str) -> ToolIoError {
    ToolIoError::ExecFailed(format!("process output capture {stage}"))
}

#[cfg(test)]
#[path = "process_capture_task_tests.rs"]
mod tests;
