//! Tick loop exit paths — send-error + cancel-during-pending-send.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use tokio_util::sync::CancellationToken;

use loopal_scheduler::{CronScheduler, ManualClock};

#[tokio::test]
async fn tick_loop_exits_when_trigger_receiver_dropped() {
    // ManualClock pinned to a time where "* * * * *" is about to fire.
    let start: DateTime<Utc> = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
    let clock = Arc::new(ManualClock::new(start));
    let sched = Arc::new(CronScheduler::with_clock(clock.clone()));
    sched
        .add("* * * * *", "will-fire", true, false)
        .await
        .expect("add");

    let (trigger_tx, trigger_rx) = tokio::sync::mpsc::channel(1);
    let cancel = CancellationToken::new();
    let handle = sched.start(trigger_tx, cancel.clone());

    // Drop receiver first so the very next send() returns Err.
    drop(trigger_rx);
    // Advance clock to force `should_fire` → mutate_tasks → send
    clock.advance(chrono::Duration::seconds(61));

    // Within a few ticks the loop must exit because send fails.
    tokio::time::timeout(Duration::from_secs(5), handle)
        .await
        .expect("tick_loop must exit after trigger_tx send error")
        .expect("join should not fail");
    // Verifies we never hit the inner `cancel.cancelled()` branch.
    assert!(!cancel.is_cancelled());
}

#[tokio::test]
async fn tick_loop_exits_when_cancel_fires_during_pending_send() {
    let start: DateTime<Utc> = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
    let clock = Arc::new(ManualClock::new(start));
    let sched = Arc::new(CronScheduler::with_clock(clock.clone()));
    sched
        .add("* * * * *", "blocked-fire", true, false)
        .await
        .expect("add");

    // Capacity 1 + a held-slot consumer makes subsequent sends pending,
    // parking the tick_loop inside `tokio::select!` on `trigger_tx.send`.
    let (trigger_tx, mut trigger_rx) = tokio::sync::mpsc::channel(1);
    let cancel = CancellationToken::new();
    let handle = sched.start(trigger_tx, cancel.clone());

    clock.advance(chrono::Duration::seconds(61));
    // Drain first trigger to leave room for the second; subsequent fires
    // will park on send since we'll stop draining after this.
    let _ = tokio::time::timeout(Duration::from_secs(2), trigger_rx.recv()).await;

    // Advance again so another trigger is produced and parks on the full
    // channel (no further recv); then cancel — inner select must exit via
    // cancel branch.
    clock.advance(chrono::Duration::seconds(61));
    tokio::time::sleep(Duration::from_millis(50)).await;
    // Fill buffer so next send parks (channel is now full because we stop
    // receiving); cancel to unblock the pending send through cancel arm.
    tokio::time::sleep(Duration::from_millis(1100)).await; // let tick hit
    cancel.cancel();

    tokio::time::timeout(Duration::from_secs(5), handle)
        .await
        .expect("tick_loop must exit after cancel")
        .expect("join should not fail");
}
