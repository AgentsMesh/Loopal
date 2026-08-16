use loopal_error::ToolIoError;
use loopal_tool_api::BgTaskConfig;

use crate::process_capture_task::{self, ProcessCaptureTask};
use crate::process_group::{KillOutcome, SpawnedChild, Termination};

pub(crate) async fn terminate_and_drain(
    spawned: &mut SpawnedChild,
    capture_task: ProcessCaptureTask,
) -> Result<Termination, ToolIoError> {
    let config = BgTaskConfig::default();
    let termination = spawned.terminate(config.sigterm_grace()).await;
    let capture = process_capture_task::join_bounded(capture_task, config.drainers_grace()).await;
    if let KillOutcome::KillFailed(error) = &termination.outcome {
        return Err(ToolIoError::ExecFailed(error.clone()));
    }
    capture?;
    Ok(termination)
}
