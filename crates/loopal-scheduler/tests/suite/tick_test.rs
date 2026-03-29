//! Deterministic tick_loop tests using ManualClock.

use std::sync::Arc;
use std::time::Duration;

use chrono::{TimeZone, Utc};
use tokio_util::sync::CancellationToken;

use loopal_scheduler::{CronScheduler, ManualClock};

/// Helper: create scheduler with ManualClock, add a task, start tick loop.
async fn setup_manual(
    cron_expr: &str,
    prompt: &str,
    recurring: bool,
    clock: Arc<ManualClock>,
) -> (
    Arc<CronScheduler>,
    tokio::sync::mpsc::Receiver<loopal_scheduler::ScheduledTrigger>,
    CancellationToken,
) {
    let sched = Arc::new(CronScheduler::with_clock(clock));
    sched.add(cron_expr, prompt, recurring).await.unwrap();
    let (trigger_tx, trigger_rx) = tokio::sync::mpsc::channel(16);
    let cancel = CancellationToken::new();
    sched.start(trigger_tx, cancel.clone());
    (sched, trigger_rx, cancel)
}

#[tokio::test(start_paused = true)]
async fn tick_fires_at_exact_minute() {
    let t0 = Utc.with_ymd_and_hms(2026, 3, 29, 10, 0, 30).unwrap();
    let clock = Arc::new(ManualClock::new(t0));

    let (_sched, mut rx, cancel) = setup_manual("* * * * *", "ping", true, clock.clone()).await;

    clock.set(Utc.with_ymd_and_hms(2026, 3, 29, 10, 1, 5).unwrap());
    for _ in 0..3 {
        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
    }

    let trigger = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("should receive trigger")
        .expect("channel open");
    assert_eq!(trigger.prompt, "ping");
    cancel.cancel();
}

#[tokio::test(start_paused = true)]
async fn tick_removes_oneshot_after_fire() {
    let t0 = Utc.with_ymd_and_hms(2026, 3, 29, 10, 0, 30).unwrap();
    let clock = Arc::new(ManualClock::new(t0));

    let (sched, mut rx, cancel) = setup_manual("* * * * *", "once", false, clock.clone()).await;

    clock.set(Utc.with_ymd_and_hms(2026, 3, 29, 10, 1, 5).unwrap());
    for _ in 0..3 {
        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
    }

    let _trigger = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("should fire")
        .expect("channel open");
    assert!(sched.list().await.is_empty(), "one-shot should be removed");
    cancel.cancel();
}

#[tokio::test(start_paused = true)]
async fn tick_expires_after_3_days() {
    let t0 = Utc.with_ymd_and_hms(2026, 3, 29, 10, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(t0));

    let (sched, _rx, cancel) = setup_manual("0 12 * * *", "daily", true, clock.clone()).await;
    assert_eq!(sched.list().await.len(), 1);

    clock.set(Utc.with_ymd_and_hms(2026, 4, 2, 10, 0, 0).unwrap());
    for _ in 0..3 {
        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
    }

    assert!(sched.list().await.is_empty(), "task should have expired");
    cancel.cancel();
}

#[tokio::test(start_paused = true)]
async fn tick_no_double_fire_same_minute() {
    let t0 = Utc.with_ymd_and_hms(2026, 3, 29, 10, 0, 30).unwrap();
    let clock = Arc::new(ManualClock::new(t0));

    let (_sched, mut rx, cancel) = setup_manual("* * * * *", "ping", true, clock.clone()).await;

    clock.set(Utc.with_ymd_and_hms(2026, 3, 29, 10, 1, 5).unwrap());
    for _ in 0..3 {
        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
    }
    let _t1 = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("first fire")
        .expect("channel open");

    clock.set(Utc.with_ymd_and_hms(2026, 3, 29, 10, 1, 30).unwrap());
    for _ in 0..3 {
        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
    }
    let result = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await;
    assert!(result.is_err(), "should not fire twice in same minute");
    cancel.cancel();
}
