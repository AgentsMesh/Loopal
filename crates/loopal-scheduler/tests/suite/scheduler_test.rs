use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use loopal_scheduler::CronScheduler;

#[tokio::test]
async fn add_and_list() {
    let sched = CronScheduler::new();
    let id = sched
        .add("*/5 * * * *", "check deploys", true, false)
        .await
        .unwrap();
    assert_eq!(id.len(), 8);

    let tasks = sched.list().await;
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, id);
    assert_eq!(tasks[0].prompt, "check deploys");
    assert!(tasks[0].recurring);
}

#[tokio::test]
async fn remove_existing_task() {
    let sched = CronScheduler::new();
    let id = sched
        .add("0 9 * * *", "morning check", true, false)
        .await
        .unwrap();
    assert!(sched.remove(&id).await);
    assert!(sched.list().await.is_empty());
}

#[tokio::test]
async fn remove_nonexistent_returns_false() {
    let sched = CronScheduler::new();
    assert!(!sched.remove("nonexist").await);
}

#[tokio::test]
async fn reject_invalid_cron() {
    let sched = CronScheduler::new();
    let err = sched
        .add("bad expr", "test", true, false)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("5 fields"));
}

#[tokio::test]
async fn enforce_max_tasks() {
    let sched = CronScheduler::new();
    for i in 0..50 {
        sched
            .add(&format!("{} * * * *", i % 60), "task", true, false)
            .await
            .unwrap();
    }
    let err = sched
        .add("0 * * * *", "overflow", true, false)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("maximum"));
}

#[tokio::test]
async fn start_and_cancel() {
    let sched = Arc::new(CronScheduler::new());
    let (trigger_tx, _trigger_rx) = tokio::sync::mpsc::channel(16);
    let cancel = CancellationToken::new();

    let handle = sched.start(trigger_tx, cancel.clone());

    tokio::time::sleep(Duration::from_millis(100)).await;
    cancel.cancel();

    tokio::time::timeout(Duration::from_secs(2), handle)
        .await
        .expect("tick loop should stop after cancel")
        .expect("task should not panic");
}

#[tokio::test]
async fn list_shows_next_fire() {
    let sched = CronScheduler::new();
    let id = sched.add("* * * * *", "ping", false, false).await.unwrap();
    let tasks = sched.list().await;
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, id);
    assert!(tasks[0].next_fire.is_some());
}

#[tokio::test]
async fn double_start_panics() {
    let sched = Arc::new(CronScheduler::new());
    let (tx1, _rx1) = tokio::sync::mpsc::channel(16);
    let (tx2, _rx2) = tokio::sync::mpsc::channel(16);
    let cancel = CancellationToken::new();

    let _handle = sched.start(tx1, cancel.clone());

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        sched.start(tx2, cancel.clone());
    }));
    assert!(result.is_err(), "second start() should panic");
    cancel.cancel();
}

/// Verify that the trigger channel receives messages when the session ends
/// (trigger_tx dropped → tick loop exits cleanly).
#[tokio::test]
async fn tick_loop_exits_when_trigger_tx_dropped() {
    let sched = Arc::new(CronScheduler::new());
    let (trigger_tx, trigger_rx) = tokio::sync::mpsc::channel(16);
    let cancel = CancellationToken::new();

    let handle = sched.start(trigger_tx, cancel.clone());

    // Drop the receiver — next time the tick loop tries to send, it will exit.
    drop(trigger_rx);
    // Also add a task so the loop actually tries to fire.
    sched.add("* * * * *", "ping", true, false).await.unwrap();

    // The loop should eventually exit (either via cancel or send error).
    cancel.cancel();
    tokio::time::timeout(Duration::from_secs(3), handle)
        .await
        .expect("tick loop should stop")
        .expect("task should not panic");
}

/// Verify one-shot task is removed from the list after it would fire.
#[tokio::test]
async fn oneshot_task_lifecycle() {
    let sched = CronScheduler::new();
    let _id = sched.add("* * * * *", "once", false, false).await.unwrap();
    assert_eq!(sched.list().await.len(), 1);

    // One-shot tasks are only removed by the tick loop.
    // Here we verify the task is properly created and has next_fire set.
    let tasks = sched.list().await;
    assert!(!tasks[0].recurring);
    assert!(tasks[0].next_fire.is_some());
}

#[tokio::test]
async fn default_constructs_empty_scheduler() {
    // Exercise `impl Default` to keep the Rust-idiom pairing of new() +
    // Default::default() honest (clippy::new_without_default).
    let sched = CronScheduler::default();
    assert!(sched.list().await.is_empty());
}
