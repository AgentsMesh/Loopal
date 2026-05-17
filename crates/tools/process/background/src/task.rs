use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Instant;

use loopal_tool_api::{HeadTail, StderrCappedBuffer};
use parking_lot::Mutex as PlMutex;
use tokio::sync::{mpsc, watch};

use crate::control::{ControlSignal, TaskStatus};

// reason: i32::MIN flags "no exit code yet" — POSIX codes are 0-255 and
// signal-killed children surface as None, so never collide.
pub const SENTINEL_NO_EXIT: i32 = i32::MIN;

pub(crate) fn decode_exit_code(raw: i32) -> Option<i32> {
    if raw == SENTINEL_NO_EXIT {
        None
    } else {
        Some(raw)
    }
}

pub struct TaskCommon {
    pub(crate) id: String,
    pub(crate) description: String,
    pub(crate) status_watch: watch::Receiver<TaskStatus>,
    pub(crate) exit_code: Arc<AtomicI32>,
    pub(crate) created_at: Instant,
    pub(crate) created_at_unix_ms: u64,
}

pub struct BackgroundTask {
    pub(crate) common: TaskCommon,
    pub(crate) control_tx: mpsc::Sender<ControlSignal>,
    pub(crate) log_path: PathBuf,
    pub(crate) stdout_head_tail: Arc<HeadTail>,
    pub(crate) stderr_buf: Arc<PlMutex<StderrCappedBuffer>>,
}

impl BackgroundTask {
    pub fn id(&self) -> &str {
        &self.common.id
    }

    pub fn description(&self) -> &str {
        &self.common.description
    }

    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    pub fn status(&self) -> TaskStatus {
        *self.common.status_watch.borrow()
    }

    pub fn exit_code(&self) -> Option<i32> {
        decode_exit_code(self.common.exit_code.load(Ordering::Acquire))
    }

    pub fn status_watch(&self) -> watch::Receiver<TaskStatus> {
        self.common.status_watch.clone()
    }

    pub fn created_at(&self) -> Instant {
        self.common.created_at
    }

    pub fn created_at_unix_ms(&self) -> u64 {
        self.common.created_at_unix_ms
    }

    pub fn is_terminal(&self) -> bool {
        self.status().is_terminal()
    }

    pub fn render_preview(&self) -> String {
        format_process_preview(&self.stdout_head_tail, &self.stderr_buf, &self.log_path)
    }
}

fn format_process_preview(
    head_tail: &Arc<HeadTail>,
    stderr_buf: &Arc<PlMutex<StderrCappedBuffer>>,
    log_path: &Path,
) -> String {
    let stdout_preview = head_tail.render_preview();
    let stdout_truncated = head_tail.was_truncated();
    let (stderr_content, stderr_truncated) = {
        let guard = stderr_buf.lock();
        (guard.snapshot(), guard.was_truncated())
    };

    let mut out = String::new();
    out.push_str("[stdout");
    if stdout_truncated {
        out.push_str(", truncated");
    }
    out.push_str("]\n");
    out.push_str(&stdout_preview);
    if !stderr_content.is_empty() {
        out.push_str("\n\n[stderr");
        if stderr_truncated {
            out.push_str(", truncated to last 8 KB");
        }
        out.push_str("]\n");
        out.push_str(&stderr_content);
    }
    out.push_str("\n\n[full log: ");
    out.push_str(&log_path.display().to_string());
    out.push(']');
    out
}
