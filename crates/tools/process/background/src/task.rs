use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Instant;

use loopal_backend::ProcessCaptureState;
use tokio::sync::{mpsc, watch};

use crate::control::{ControlSignal, TaskStatus};

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
    pub(crate) capture_state: Arc<ProcessCaptureState>,
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
        self.capture_state.render_preview()
    }

    pub fn render_output(&self, wait_timed_out: bool) -> String {
        self.capture_state.render_output(wait_timed_out)
    }
}
