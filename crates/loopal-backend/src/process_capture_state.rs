use std::path::PathBuf;
use std::sync::Arc;

use loopal_tool_api::ProcessOutputSanitizer;
use parking_lot::Mutex;
use tokio::sync::watch;
use zeroize::Zeroizing;

use crate::process_capture_render::{guard, guard_with_suffix, render_preview, status_text};

#[derive(Clone)]
pub struct ProcessCaptureSnapshot {
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

#[derive(Clone, Copy)]
pub enum ProcessCompletion {
    Completed,
    Failed,
    Killed,
}

pub struct ProcessCaptureState {
    log_path: PathBuf,
    inner: Mutex<CaptureView>,
    capture_failed: watch::Sender<bool>,
}

struct CaptureView {
    snapshot: ProcessCaptureSnapshot,
    preview: String,
    running: String,
    running_wait_timeout: String,
    terminal: Option<String>,
    sanitizer: Option<Arc<dyn ProcessOutputSanitizer>>,
}

impl ProcessCaptureState {
    pub(crate) fn new(
        log_path: PathBuf,
        sanitizer: Option<Arc<dyn ProcessOutputSanitizer>>,
    ) -> Arc<Self> {
        let raw_preview = render_preview("", false, "", false, &log_path);
        let preview = guard(&sanitizer, &raw_preview);
        let (capture_failed, _) = watch::channel(false);
        Arc::new(Self {
            log_path,
            capture_failed,
            inner: Mutex::new(CaptureView {
                snapshot: ProcessCaptureSnapshot {
                    stdout: String::new(),
                    stderr: String::new(),
                    stdout_truncated: false,
                    stderr_truncated: false,
                },
                running: guard_with_suffix(&sanitizer, &preview, "\n[Status: Running]"),
                running_wait_timeout: guard_with_suffix(
                    &sanitizer,
                    &preview,
                    "\n[Status: Running (timed out waiting)]",
                ),
                preview,
                terminal: None,
                sanitizer,
            }),
        })
    }

    pub(crate) fn publish(
        &self,
        stdout: Zeroizing<String>,
        stdout_truncated: bool,
        stderr: Zeroizing<String>,
        stderr_truncated: bool,
        progress: &str,
    ) -> String {
        let mut inner = self.inner.lock();
        let raw_preview = render_preview(
            &stdout,
            stdout_truncated,
            &stderr,
            stderr_truncated,
            &self.log_path,
        );
        let preview = guard(&inner.sanitizer, &raw_preview);
        inner.snapshot = ProcessCaptureSnapshot {
            stdout: guard(&inner.sanitizer, &stdout),
            stderr: guard(&inner.sanitizer, &stderr),
            stdout_truncated,
            stderr_truncated,
        };
        inner.running = guard_with_suffix(&inner.sanitizer, &raw_preview, "\n[Status: Running]");
        inner.running_wait_timeout = guard_with_suffix(
            &inner.sanitizer,
            &raw_preview,
            "\n[Status: Running (timed out waiting)]",
        );
        inner.preview = preview;
        guard(&inner.sanitizer, progress)
    }

    pub fn snapshot(&self) -> ProcessCaptureSnapshot {
        self.inner.lock().snapshot.clone()
    }

    pub fn render_preview(&self) -> String {
        self.inner.lock().preview.clone()
    }

    pub fn render_output(&self, wait_timed_out: bool) -> String {
        let inner = self.inner.lock();
        if let Some(terminal) = &inner.terminal {
            return terminal.clone();
        }
        if wait_timed_out {
            inner.running_wait_timeout.clone()
        } else {
            inner.running.clone()
        }
    }

    pub fn capture_failed(&self) -> bool {
        *self.capture_failed.borrow()
    }

    pub async fn wait_for_capture_failure(&self) {
        let mut receiver = self.capture_failed.subscribe();
        let _ = receiver.wait_for(|failed| *failed).await;
    }

    pub fn record_capture_failure(&self) {
        self.capture_failed.send_replace(true);
    }

    pub fn finalize(&self, completion: ProcessCompletion, exit_code: Option<i32>) {
        let mut inner = self.inner.lock();
        let status = status_text(completion, exit_code);
        inner.terminal = Some(guard_with_suffix(
            &inner.sanitizer,
            &inner.preview,
            &format!("\n{status}"),
        ));
        inner.sanitizer = None;
    }
}
