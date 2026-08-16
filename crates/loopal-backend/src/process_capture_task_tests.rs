use std::time::Duration;

use loopal_error::ToolIoError;

use super::{ProcessCaptureTask, join, join_bounded};

#[tokio::test]
async fn bounded_join_aborts_a_stalled_capture() {
    let task = ProcessCaptureTask::spawn(std::future::pending::<Result<(), ToolIoError>>(), || {});

    let error = join_bounded(task, Duration::ZERO).await.unwrap_err();
    assert!(error.to_string().contains("drain timed out"));
}

#[tokio::test]
async fn join_reports_an_externally_cancelled_capture() {
    let task = ProcessCaptureTask::spawn(std::future::pending::<Result<(), ToolIoError>>(), || {});
    task.handle.as_ref().unwrap().abort();

    let error = join(task).await.unwrap_err();
    assert!(error.to_string().contains("task cancelled"));
}
