use std::sync::Arc;
use std::time::Duration;

use loopal_tool_api::ToolResult;
use tokio::sync::oneshot;

use crate::control::{StopOutcome, StoreError, TaskStatus};
use crate::store::BackgroundTaskStore;

#[cfg(test)]
#[path = "ops_tests.rs"]
mod tests;

pub async fn bg_output(
    store: &Arc<BackgroundTaskStore>,
    process_id: &str,
    block: bool,
    timeout: Duration,
) -> ToolResult {
    let Some(mut watch_rx) = store.read_task(process_id, |t| t.status_watch()) else {
        return ToolResult::error(format!("Process not found: {process_id}"));
    };

    if block {
        let wait = async {
            loop {
                if watch_rx.borrow().is_terminal() {
                    return;
                }
                if watch_rx.changed().await.is_err() {
                    return;
                }
            }
        };
        if tokio::time::timeout(timeout, wait).await.is_err() {
            let output = store
                .read_task(process_id, |task| task.render_output(true))
                .unwrap_or_default();
            return ToolResult::success(output);
        }
    }

    let Some((status, output)) = store.read_task(process_id, |task| {
        (task.status(), task.render_output(false))
    }) else {
        return ToolResult::error(format!("Process not found: {process_id}"));
    };
    if matches!(status, TaskStatus::Running | TaskStatus::Completed) {
        ToolResult::success(output)
    } else {
        ToolResult::error(output)
    }
}

pub async fn bg_stop(store: &Arc<BackgroundTaskStore>, process_id: &str) -> ToolResult {
    let (ack_tx, ack_rx) = oneshot::channel();
    match store.send_stop(process_id, ack_tx) {
        Err(StoreError::NotFound) => {
            return ToolResult::error(format!("Process not found: {process_id}"));
        }
        Err(StoreError::AlreadyTerminal { status, exit_code }) => {
            return ToolResult::success(format!(
                "Process already {status:?}: {process_id} (exit {exit_code:?})"
            ));
        }
        Err(StoreError::ChannelClosed) => {
            // reason: race window — monitor exited (status went terminal) in
            // the gap between status check and try_send. Status is now final;
            // retry to observe the actual terminal status + exit code.
            return ToolResult::error(format!(
                "Process {process_id} transitioned to terminal during stop — retry to see final status"
            ));
        }
        Ok(()) => {}
    }

    match tokio::time::timeout(store.config().stop_ack_timeout(), ack_rx).await {
        Ok(Ok(StopOutcome::Killed { exit_code })) => {
            ToolResult::success(format!("Killed: {process_id} (exit {exit_code:?})"))
        }
        Ok(Ok(StopOutcome::KillFailed(e))) => {
            ToolResult::error(format!("Kill failed for {process_id}: {e}"))
        }
        Ok(Err(_)) => ToolResult::error(format!(
            "Stop ack channel dropped before reply: {process_id}"
        )),
        Err(_) => ToolResult::error(format!("Stop ack timed out: {process_id}")),
    }
}
