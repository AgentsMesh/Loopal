use std::io;
use std::process::ExitStatus;
use std::time::Duration;

use crate::{ProcessCaptureState, SpawnedChild};

pub(crate) enum WaitOutcome {
    Exited(io::Result<ExitStatus>),
    CaptureFailed,
    TimedOut,
}

pub(crate) async fn wait(
    child: &mut SpawnedChild,
    capture: &ProcessCaptureState,
    timeout: Duration,
) -> WaitOutcome {
    tokio::select! {
        biased;
        _ = capture.wait_for_capture_failure() => WaitOutcome::CaptureFailed,
        result = child.wait() => WaitOutcome::Exited(result),
        _ = tokio::time::sleep(timeout) => WaitOutcome::TimedOut,
    }
}
