use loopal_tool_api::ExecResult;

use crate::process_capture_state::ProcessCaptureState;

pub(crate) fn exec_result(
    state: &ProcessCaptureState,
    exit_code: i32,
    log_path: std::path::PathBuf,
) -> ExecResult {
    let snapshot = state.snapshot();
    ExecResult {
        stdout: snapshot.stdout,
        stderr: snapshot.stderr,
        stdout_truncated: snapshot.stdout_truncated,
        stderr_truncated: snapshot.stderr_truncated,
        exit_code,
        log_path,
    }
}
