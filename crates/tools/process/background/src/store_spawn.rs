use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicI32;
use std::time::Instant;

use loopal_backend::SpawnedChild;
use loopal_tool_api::{HeadTail, StderrCappedBuffer};
use parking_lot::Mutex as PlMutex;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::clock::unix_ms;
use crate::control::TaskStatus;
use crate::monitor::{MonitorTiming, run_process_monitor};
use crate::monitor_watchdog::install_panic_watchdog;
use crate::store::{BackgroundTaskStore, CONTROL_QUEUE_CAP};
use crate::task::{BackgroundTask, SENTINEL_NO_EXIT, TaskCommon};

impl BackgroundTaskStore {
    pub fn spawn_process_task(
        &self,
        spawned: SpawnedChild,
        log_path: PathBuf,
        stdout_head_tail: Arc<HeadTail>,
        stderr_buf: Arc<PlMutex<StderrCappedBuffer>>,
        output_drainers: Vec<JoinHandle<()>>,
        desc: &str,
    ) -> String {
        let id = self.generate_task_id();
        let (status_tx, status_rx) = watch::channel(TaskStatus::Running);
        let (control_tx, control_rx) = mpsc::channel(CONTROL_QUEUE_CAP);
        let exit_code = Arc::new(AtomicI32::new(SENTINEL_NO_EXIT));

        let common = TaskCommon {
            id: id.clone(),
            description: desc.to_string(),
            status_watch: status_rx,
            exit_code: exit_code.clone(),
            created_at: Instant::now(),
            created_at_unix_ms: unix_ms(),
        };
        self.insert(BackgroundTask {
            common,
            control_tx,
            log_path,
            stdout_head_tail,
            stderr_buf,
        });

        let timing = MonitorTiming {
            drainers_grace: self.config().drainers_grace(),
            sigterm_grace: self.config().sigterm_grace(),
        };
        // The watchdog only needs to abort drainers if the monitor task
        // itself panicked — it doesn't claim ownership.
        let watchdog_aborts: Vec<_> = output_drainers.iter().map(|h| h.abort_handle()).collect();
        let monitor_handle = tokio::spawn(run_process_monitor(
            spawned.child,
            spawned.pgid,
            output_drainers,
            exit_code,
            status_tx,
            control_rx,
            timing,
        ));
        install_panic_watchdog(monitor_handle, watchdog_aborts, id.clone());
        id
    }
}
