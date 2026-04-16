//! Background task monitoring — store insertion, monitor spawning, I/O helpers.

use std::sync::{Arc, Mutex};

use loopal_tool_background::{BackgroundTask, BackgroundTaskStore, TaskStatus};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::task::AbortHandle;

pub fn insert_task(store: &BackgroundTaskStore, task_id: &str, task: BackgroundTask) {
    store.insert(task_id.to_string(), task);
}

/// Build a `BackgroundTask` from its component Arc fields.
pub fn build_task(
    output: &Arc<Mutex<String>>,
    exit_code: &Arc<Mutex<Option<i32>>>,
    status: &Arc<Mutex<TaskStatus>>,
    child: &Arc<Mutex<Option<tokio::process::Child>>>,
    desc: &str,
    watch_rx: tokio::sync::watch::Receiver<TaskStatus>,
) -> BackgroundTask {
    BackgroundTask {
        output: Arc::clone(output),
        exit_code: Arc::clone(exit_code),
        status: Arc::clone(status),
        description: desc.to_string(),
        child: Arc::clone(child),
        status_watch: watch_rx,
    }
}

/// Monitor with reader task cleanup.
///
/// After the child exits, waits briefly for reader tasks to drain (pipes
/// should close), then aborts any stragglers to prevent task leaks.
/// Only updates status if it is still `Running` — respects `bg_stop`'s
/// prior write of `Failed`.
pub fn spawn_monitor_with_cleanup<F>(
    child_arc: Arc<Mutex<Option<tokio::process::Child>>>,
    combined_output: Arc<Mutex<String>>,
    exit_code: Arc<Mutex<Option<i32>>>,
    status: Arc<Mutex<TaskStatus>>,
    watch_tx: tokio::sync::watch::Sender<TaskStatus>,
    abort_handles: Vec<AbortHandle>,
    refresh_output: F,
) where
    F: FnOnce() -> String + Send + 'static,
{
    tokio::spawn(async move {
        let mut ch = match child_arc.lock().unwrap().take() {
            Some(c) => c,
            None => return,
        };
        let code = ch.wait().await.ok().and_then(|s| s.code());

        // Pipes close with child exit — give readers a brief window to
        // finish draining, then abort any stragglers.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        for handle in &abort_handles {
            handle.abort();
        }

        let output = refresh_output();
        *combined_output.lock().unwrap() = output;
        *exit_code.lock().unwrap() = code;

        let final_status = if code == Some(0) {
            TaskStatus::Completed
        } else {
            TaskStatus::Failed
        };
        let mut s = status.lock().unwrap();
        if *s == TaskStatus::Running {
            *s = final_status;
        }
        let current = s.clone();
        drop(s);
        let _ = watch_tx.send(current);
    });
}

pub fn combine(stdout: &Mutex<String>, stderr: &Mutex<String>) -> String {
    let out = stdout.lock().unwrap().clone();
    let err = stderr.lock().unwrap().clone();
    if err.is_empty() {
        return out;
    }
    if out.is_empty() {
        return err;
    }
    format!("{out}\n{err}")
}

pub fn truncate_cmd(cmd: &str, max: usize) -> String {
    let single_line: String = cmd.split_whitespace().collect::<Vec<_>>().join(" ");
    if single_line.len() <= max {
        single_line
    } else {
        let mut end = max.saturating_sub(1);
        while end > 0 && !single_line.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &single_line[..end])
    }
}

pub async fn read_pipe<R: tokio::io::AsyncRead + Unpin>(buf: &Mutex<String>, reader: R) {
    let mut br = BufReader::new(reader);
    let mut line = String::new();
    loop {
        line.clear();
        match br.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => buf.lock().unwrap().push_str(&line),
            Err(_) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_cmd_ascii_within_limit() {
        assert_eq!(truncate_cmd("ls -la", 10), "ls -la");
    }

    #[test]
    fn truncate_cmd_ascii_exceeds_limit() {
        let result = truncate_cmd("echo hello world foo bar", 10);
        assert!(result.ends_with('…'));
        // 9 ASCII chars + 3-byte '…' = 12 bytes max
        assert!(result.len() <= 12);
    }

    #[test]
    fn truncate_cmd_multibyte_boundary() {
        // '创' = 3 bytes, so "echo 创建目录" has byte offsets where max could land mid-char
        let result = truncate_cmd("echo 创建目录结构并初始化配置文件", 12);
        assert!(result.ends_with('…'));
        // Strip the trailing '…' (3 bytes) and verify the prefix is valid UTF-8
        let prefix = &result[..result.len() - '…'.len_utf8()];
        assert!(prefix.is_char_boundary(prefix.len()));
    }

    #[test]
    fn truncate_cmd_max_exactly_inside_char() {
        // "创" is bytes 0..3; max=2 lands inside it, must back up to 0
        let result = truncate_cmd("创建", 2);
        assert_eq!(result, "…"); // backed up to 0, only ellipsis remains
    }

    #[test]
    fn truncate_cmd_collapses_whitespace() {
        assert_eq!(truncate_cmd("ls  -la   /tmp", 20), "ls -la /tmp");
    }

    #[test]
    fn truncate_cmd_zero_max() {
        let result = truncate_cmd("hello", 0);
        assert_eq!(result, "…");
    }
}
