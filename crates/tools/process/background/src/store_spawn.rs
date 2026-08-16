use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicI32;
use std::time::Instant;

use loopal_backend::{ProcessCaptureState, ProcessCaptureTask, SpawnedChild};
use tokio::sync::{mpsc, watch};

use crate::clock::unix_ms;
use crate::control::TaskStatus;
use crate::monitor::{MonitorTiming, run_process_monitor};
use crate::store::{BackgroundTaskStore, CONTROL_QUEUE_CAP};
use crate::task::{BackgroundTask, SENTINEL_NO_EXIT, TaskCommon};

impl BackgroundTaskStore {
    pub fn spawn_process_task(
        &self,
        spawned: SpawnedChild,
        log_path: PathBuf,
        capture_state: Arc<ProcessCaptureState>,
        capture_task: ProcessCaptureTask,
        description: &str,
    ) -> String {
        let id = self.generate_task_id();
        let (status_tx, status_rx) = watch::channel(TaskStatus::Running);
        let (control_tx, control_rx) = mpsc::channel(CONTROL_QUEUE_CAP);
        let exit_code = Arc::new(AtomicI32::new(SENTINEL_NO_EXIT));
        let common = TaskCommon {
            id: id.clone(),
            description: description.to_string(),
            status_watch: status_rx,
            exit_code: exit_code.clone(),
            created_at: Instant::now(),
            created_at_unix_ms: unix_ms(),
        };
        let task = BackgroundTask {
            common,
            control_tx,
            log_path,
            capture_state: capture_state.clone(),
        };
        let timing = MonitorTiming {
            drainers_grace: self.config().drainers_grace(),
            sigterm_grace: self.config().sigterm_grace(),
        };
        tokio::spawn(run_process_monitor(
            spawned,
            capture_task,
            capture_state,
            exit_code,
            status_tx,
            control_rx,
            timing,
        ));
        self.insert(task);
        id
    }
}
