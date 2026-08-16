use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::stop_background_bridge;

struct DropSignal(Arc<AtomicBool>);

impl Drop for DropSignal {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test(start_paused = true)]
async fn stuck_bridge_is_aborted_after_the_grace_period() {
    let dropped = Arc::new(AtomicBool::new(false));
    let task_dropped = dropped.clone();
    let bridge_task = tokio::spawn(async move {
        let _drop_signal = DropSignal(task_dropped);
        std::future::pending::<()>().await;
    });
    tokio::task::yield_now().await;
    let bridge_abort = bridge_task.abort_handle();

    stop_background_bridge(bridge_task, bridge_abort).await;
    tokio::task::yield_now().await;

    assert!(dropped.load(Ordering::SeqCst));
}
