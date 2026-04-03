//! Background process operations — query output and stop running tasks.

use std::sync::Arc;
use std::time::Duration;

use loopal_tool_api::ToolResult;
use loopal_tool_background::{BackgroundTaskStore, TaskStatus};

/// Read output from a background process (blocking or non-blocking).
pub async fn bg_output(
    store: &BackgroundTaskStore,
    process_id: &str,
    block: bool,
    timeout_ms: u64,
) -> ToolResult {
    let cloned = store.with_task(process_id, |task| {
        (
            Arc::clone(&task.output),
            Arc::clone(&task.exit_code),
            Arc::clone(&task.status),
            task.status_watch.clone(),
        )
    });
    let Some((output_buf, exit_code_buf, status_buf, mut watch_rx)) = cloned else {
        return ToolResult::error(format!("Process not found: {process_id}"));
    };

    if block {
        let deadline = Duration::from_millis(timeout_ms);
        let wait = async {
            loop {
                if *watch_rx.borrow() != TaskStatus::Running {
                    return;
                }
                if watch_rx.changed().await.is_err() {
                    return;
                }
            }
        };
        if tokio::time::timeout(deadline, wait).await.is_err() {
            let output = output_buf.lock().unwrap().clone();
            return ToolResult::success(format!("{output}\n[Status: Running (timed out waiting)]"));
        }
    }

    let output = output_buf.lock().unwrap().clone();
    let status = status_buf.lock().unwrap().clone();
    let exit_code = *exit_code_buf.lock().unwrap();
    format_bg_status(&output, &status, exit_code)
}

/// Stop a background process.
///
/// Lock order: `child` → `status` (matches the monitor task).
/// Always returns success — even if the monitor already set a terminal
/// status (race between kill and monitor is benign).
pub fn bg_stop(store: &BackgroundTaskStore, process_id: &str) -> ToolResult {
    let result = store.with_task(process_id, |task| {
        // Kill child (if monitor hasn't taken it already).
        {
            if let Some(child) = task.child.lock().unwrap().as_mut() {
                let _ = child.start_kill();
            }
        }

        // Force status to Failed if still Running.
        let mut status = task.status.lock().unwrap();
        if *status == TaskStatus::Running {
            *status = TaskStatus::Failed;
        }
    });

    if result.is_none() {
        return ToolResult::error(format!("Process not found: {process_id}"));
    }
    ToolResult::success(format!("Process stopped: {process_id}"))
}

fn format_bg_status(output: &str, status: &TaskStatus, exit_code: Option<i32>) -> ToolResult {
    match status {
        TaskStatus::Running => ToolResult::success(format!("{output}\n[Status: Running]")),
        TaskStatus::Completed => match exit_code {
            Some(c) => ToolResult::success(format!("{output}\n[Completed, exit {c}]")),
            None => ToolResult::success(format!("{output}\n[Status: Completed]")),
        },
        TaskStatus::Failed => match exit_code {
            Some(c) => ToolResult::error(format!("{output}\n[Failed, exit {c}]")),
            None => ToolResult::error(format!("{output}\n[Status: Failed]")),
        },
    }
}
