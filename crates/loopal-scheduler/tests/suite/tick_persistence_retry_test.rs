use std::sync::Arc;
use std::time::Duration;

use chrono::{TimeZone, Utc};
use tokio_util::sync::CancellationToken;

use loopal_scheduler::{CronScheduler, ManualClock, SessionScopedCronStorage};

use crate::mock_storage::MockStorage;

#[tokio::test(start_paused = true)]
async fn save_failure_retries_on_next_tick() {
    let t0 = Utc.with_ymd_and_hms(2026, 4, 10, 10, 0, 30).unwrap();
    let clock = Arc::new(ManualClock::new(t0));
    let store = MockStorage::new();
    let store_dyn: Arc<dyn SessionScopedCronStorage> = store.clone();
    let sched = Arc::new(CronScheduler::with_session_storage_and_clock(
        store_dyn,
        clock.clone(),
    ));
    sched.switch_session("retry-test").await.unwrap();

    store.arm_save_failure();
    sched.add("* * * * *", "p", true, true).await.expect("add");
    sched.wait_idle().await;
    assert_eq!(store.fail_save_attempts(), 1);
    assert_eq!(store.save_count().await, 0);

    let (tx, _rx) = tokio::sync::mpsc::channel(16);
    let cancel = CancellationToken::new();
    sched.start(tx, cancel.clone());

    clock.set(Utc.with_ymd_and_hms(2026, 4, 10, 10, 0, 45).unwrap());
    for _ in 0..3 {
        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
    }
    sched.wait_idle().await;

    assert!(
        store.save_count().await >= 1,
        "dirty flag must trigger retry"
    );
    cancel.cancel();
}
