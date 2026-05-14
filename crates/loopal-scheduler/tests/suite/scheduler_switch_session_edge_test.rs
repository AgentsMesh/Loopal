use std::sync::Arc;

use chrono::{TimeZone, Utc};

use loopal_scheduler::{CronScheduler, ManualClock, PersistedTask, SessionScopedCronStorage};

use crate::mock_storage::MockStorage;

#[tokio::test]
async fn switch_resets_store_disabled_for_new_session() {
    // After a corrupt-load on session A latches store_disabled,
    // switching to B must clear the latch so B's I/O works normally.
    let now = Utc.with_ymd_and_hms(2026, 4, 10, 12, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(now));
    let storage = MockStorage::new();
    storage
        .seed(
            "beta",
            vec![PersistedTask {
                id: "b1".into(),
                cron: "*/5 * * * *".into(),
                prompt: "hi".into(),
                recurring: true,
                created_at_unix_ms: now.timestamp_millis(),
                last_fired_unix_ms: Some(now.timestamp_millis()),
            }],
        )
        .await;
    storage.arm_load_failure_once_for("alpha").await;
    let store_dyn: Arc<dyn SessionScopedCronStorage> = storage.clone();
    let scheduler = CronScheduler::with_session_storage_and_clock(store_dyn, clock);
    let _ = scheduler.switch_session("alpha").await;
    let n = scheduler.switch_session("beta").await.expect("beta loads");
    assert_eq!(n, 1);
}
