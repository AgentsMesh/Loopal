//! Output formatting for foreground command results.

use loopal_tool_api::{
    DEFAULT_MAX_OUTPUT_BYTES, DEFAULT_MAX_OUTPUT_LINES, ToolResult, backend_types::ExecResult,
    truncate_output,
};

/// Format a foreground `ExecResult` into a `ToolResult`.
pub fn format_exec_result(output: ExecResult) -> ToolResult {
    let mut combined = String::new();
    if !output.stdout.is_empty() {
        combined.push_str(&output.stdout);
    }
    if !output.stderr.is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(&output.stderr);
    }
    let truncated = truncate_output(
        &combined,
        DEFAULT_MAX_OUTPUT_LINES,
        DEFAULT_MAX_OUTPUT_BYTES,
    );
    if output.exit_code != 0 {
        ToolResult::error(format!("Exit code: {}\n{truncated}", output.exit_code))
    } else {
        ToolResult::success(truncated)
    }
}

/// Format a timeout-to-background conversion into a success `ToolResult`.
pub fn format_converted_to_background(
    task_id: &str,
    timeout: std::time::Duration,
    partial_output: &str,
) -> ToolResult {
    let timeout_secs = timeout.as_secs();
    let mut msg = format!(
        "Command timed out after {timeout_secs}s and was moved to background.\n\
         process_id: {task_id}\n\
         Use Bash with process_id to check output later."
    );
    if !partial_output.is_empty() {
        let truncated = truncate_output(
            partial_output,
            DEFAULT_MAX_OUTPUT_LINES,
            DEFAULT_MAX_OUTPUT_BYTES,
        );
        msg.push_str("\n\nPartial output before timeout:\n");
        msg.push_str(&truncated);
    }
    ToolResult::success(msg)
}
