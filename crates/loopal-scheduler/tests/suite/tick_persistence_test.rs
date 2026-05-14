use std::sync::Arc;
use std::time::Duration;

use chrono::{TimeZone, Utc};
use tokio_util::sync::CancellationToken;

use loopal_scheduler::{CronScheduler, ManualClock, SessionScopedCronStorage};

use crate::mock_storage::MockStorage;

async fn build(store: Arc<MockStorage>, clock: Arc<ManualClock>) -> Arc<CronScheduler> {
    let store_dyn: Arc<dyn SessionScopedCronStorage> = store;
    let sched = Arc::new(CronScheduler::with_session_storage_and_clock(
        store_dyn, clock,
    ));
    sched.switch_session("tick-test").await.unwrap();
    sched
}

async fn pump_ticks(clock: &ManualClock, to: chrono::DateTime<chrono::Utc>, rounds: usize) {
    clock.set(to);
    for _ in 0..rounds {
        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
    }
}

#[tokio::test(start_paused = true)]
async fn recurring_durable_fire_persists_last_fired() {
    let t0 = Utc.with_ymd_and_hms(2026, 4, 10, 10, 0, 30).unwrap();
    let clock = Arc::new(ManualClock::new(t0));
    let store = MockStorage::new();
    let sched = build(store.clone(), clock.clone()).await;
    sched.add("* * * * *", "p", true, true).await.expect("add");
    sched.wait_idle().await;
    let initial_saves = store.save_count().await;
    assert_eq!(initial_saves, 1);

    let (tx, mut rx) = tokio::sync::mpsc::channel(16);
    let cancel = CancellationToken::new();
    sched.start(tx, cancel.clone());

    pump_ticks(
        &clock,
        Utc.with_ymd_and_hms(2026, 4, 10, 10, 1, 5).unwrap(),
        3,
    )
    .await;
    let _trigger = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("fire")
        .expect("open");
    sched.wait_idle().await;

    assert!(store.save_count().await > initial_saves, "fire must save");
    let last = store.last_save().await;
    assert_eq!(last.len(), 1, "one task survives");
    assert!(
        last[0].last_fired_unix_ms.is_some(),
        "last_fired must be persisted"
    );
    cancel.cancel();
}

#[tokio::test(start_paused = true)]
async fn oneshot_durable_fire_removes_from_store() {
    let t0 = Utc.with_ymd_and_hms(2026, 4, 10, 10, 0, 30).unwrap();
    let clock = Arc::new(ManualClock::new(t0));
    let store = MockStorage::new();
    let sched = build(store.clone(), clock.clone()).await;
    sched
        .add("* * * * *", "once", false, true)
        .await
        .expect("add");

    let (tx, mut rx) = tokio::sync::mpsc::channel(16);
    let cancel = CancellationToken::new();
    sched.start(tx, cancel.clone());

    pump_ticks(
        &clock,
        Utc.with_ymd_and_hms(2026, 4, 10, 10, 1, 5).unwrap(),
        3,
    )
    .await;
    let _trigger = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("fire")
        .expect("open");

    assert!(store.last_save().await.is_empty(), "one-shot removed");
    cancel.cancel();
}

#[tokio::test(start_paused = true)]
async fn multiple_fires_in_one_tick_save_once() {
    let t0 = Utc.with_ymd_and_hms(2026, 4, 10, 10, 0, 30).unwrap();
    let clock = Arc::new(ManualClock::new(t0));
    let store = MockStorage::new();
    let sched = build(store.clone(), clock.clone()).await;
    sched.add("* * * * *", "a", true, true).await.expect("add");
    sched.add("* * * * *", "b", true, true).await.expect("add");
    sched.add("* * * * *", "c", true, true).await.expect("add");
    sched.wait_idle().await;
    let baseline = store.save_count().await;

    let (tx, mut rx) = tokio::sync::mpsc::channel(16);
    let cancel = CancellationToken::new();
    sched.start(tx, cancel.clone());

    pump_ticks(
        &clock,
        Utc.with_ymd_and_hms(2026, 4, 10, 10, 1, 5).unwrap(),
        3,
    )
    .await;
    for _ in 0..3 {
        let _ = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;
    }
    sched.wait_idle().await;

    let after = store.save_count().await;
    assert_eq!(after - baseline, 1, "batched fires must save once");
    cancel.cancel();
}
