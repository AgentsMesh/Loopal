use tokio::task::{AbortHandle, JoinHandle};

// reason: if monitor panics, drainers would otherwise keep the log_writer
// Arc alive forever and leak the FD. The watchdog force-aborts them.
pub(crate) fn install_panic_watchdog(
    monitor_handle: JoinHandle<()>,
    panic_safe_drainers: Vec<AbortHandle>,
    task_id: String,
) {
    tokio::spawn(async move {
        if let Err(join_err) = monitor_handle.await
            && join_err.is_panic()
        {
            for h in panic_safe_drainers {
                h.abort();
            }
            tracing::error!(
                task_id = %task_id,
                "bg monitor task panicked, drainers force-aborted"
            );
        }
    });
}
