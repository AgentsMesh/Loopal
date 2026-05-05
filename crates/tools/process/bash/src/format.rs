use loopal_tool_api::{
    DEFAULT_MAX_OUTPUT_BYTES, DEFAULT_MAX_OUTPUT_LINES, ToolResult, backend_types::ExecResult,
    extract_overflow_path, humanize_size, needs_truncation, truncate_output,
};

use crate::strategy::{StrategyOutcome, apply, detect_strategy};

const METADATA_THRESHOLD: usize = 1024;

pub fn format_exec_result(output: ExecResult, command: &str) -> ToolResult {
    let (stdout_body, stdout_overflow) = extract_overflow_path(&output.stdout);
    let (stderr_body, stderr_overflow) = extract_overflow_path(&output.stderr);
    let cleaned = ExecResult {
        stdout: stdout_body,
        stderr: stderr_body,
        exit_code: output.exit_code,
    };

    let stdout_size = cleaned.stdout.len();
    let stderr_size = cleaned.stderr.len();
    let needs_meta = cleaned.exit_code != 0
        || stdout_size > METADATA_THRESHOLD
        || stderr_size > METADATA_THRESHOLD
        || stdout_overflow.is_some()
        || stderr_overflow.is_some();
    let needs_strategy = needs_truncation(
        &cleaned.stdout,
        DEFAULT_MAX_OUTPUT_LINES,
        DEFAULT_MAX_OUTPUT_BYTES,
    ) || needs_truncation(
        &cleaned.stderr,
        DEFAULT_MAX_OUTPUT_LINES,
        DEFAULT_MAX_OUTPUT_BYTES,
    );

    let outcome = if needs_strategy {
        let strategy = detect_strategy(&cleaned, command);
        apply(strategy, &cleaned)
    } else {
        StrategyOutcome {
            stdout: cleaned.stdout.clone(),
            stderr: cleaned.stderr.clone(),
            applied: None,
            hint: None,
        }
    };

    let body = if needs_meta {
        format_with_metadata(
            cleaned.exit_code,
            stdout_size,
            stderr_size,
            stdout_overflow.as_deref(),
            stderr_overflow.as_deref(),
            &outcome,
        )
    } else {
        merge_simple(&outcome)
    };

    if cleaned.exit_code != 0 {
        ToolResult::error(body)
    } else {
        ToolResult::success(body)
    }
}

fn merge_simple(outcome: &StrategyOutcome) -> String {
    let mut body = outcome.stdout.clone();
    if !outcome.stderr.is_empty() {
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
        body.push_str(&outcome.stderr);
    }
    body
}

fn format_with_metadata(
    exit_code: i32,
    stdout_size: usize,
    stderr_size: usize,
    stdout_overflow: Option<&str>,
    stderr_overflow: Option<&str>,
    outcome: &StrategyOutcome,
) -> String {
    let mut buf = String::new();
    buf.push_str(&format!("exit_code: {exit_code}\n"));
    buf.push_str(&format!("stdout_size: {}\n", humanize_size(stdout_size)));
    buf.push_str(&format!("stderr_size: {}\n", humanize_size(stderr_size)));
    if let Some(p) = stdout_overflow {
        buf.push_str(&format!("stdout_overflow: {p}\n"));
    }
    if let Some(p) = stderr_overflow {
        buf.push_str(&format!("stderr_overflow: {p}\n"));
    }
    if let Some(applied) = outcome.applied {
        buf.push_str(&format!("applied: {applied}\n"));
    }
    if let Some(hint) = outcome.hint {
        buf.push_str(&format!("hint: '{hint}'\n"));
    }
    buf.push('\n');

    if !outcome.stderr.is_empty() {
        buf.push_str("--- stderr ---\n");
        buf.push_str(&outcome.stderr);
        if !outcome.stderr.ends_with('\n') {
            buf.push('\n');
        }
    }
    if !outcome.stdout.is_empty() {
        buf.push_str("--- stdout ---\n");
        buf.push_str(&outcome.stdout);
    }
    buf
}

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
