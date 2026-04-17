//! Edge-case tests for cron_bridge — diff-skip invariants + failure paths.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use loopal_scheduler::CronScheduler;

use super::cron_bridge_helpers::{
    CaptureFrontend, FailingFrontend, TEST_INTERVAL, count_cron_events,
};

#[tokio::test]
async fn frontend_emit_errors_do_not_crash_bridge() {
    let scheduler = Arc::new(CronScheduler::new());
    let (frontend, emit_calls) = FailingFrontend::new();
    let bridge = loopal_agent_server::testing::cron_bridge_spawn_with_interval(
        scheduler.clone(),
        Arc::new(frontend),
        TEST_INTERVAL,
    );

    // Drive one change so the bridge tries to emit more than the initial frame.
    tokio::time::sleep(Duration::from_millis(80)).await;
    scheduler
        .add("*/5 * * * *", "job", true, false)
        .await
        .expect("add");
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Bridge must still be running despite repeated emit errors.
    assert!(
        !bridge.is_finished(),
        "bridge must keep running despite emit errors"
    );
    // And it must have attempted multiple emits.
    assert!(emit_calls.load(Ordering::SeqCst) >= 2);
    bridge.abort();
}

#[tokio::test]
async fn identical_job_set_skips_emit() {
    let scheduler = Arc::new(CronScheduler::new());
    scheduler
        .add("*/5 * * * *", "stable job", true, false)
        .await
        .expect("add");
    let (frontend, events) = CaptureFrontend::new();
    let bridge = loopal_agent_server::testing::cron_bridge_spawn_with_interval(
        scheduler.clone(),
        Arc::new(frontend),
        TEST_INTERVAL,
    );

    // Let several intervals pass with no job-set change.
    // Since diff is (id, prompt, recurring) — not next_fire — no extra emits.
    tokio::time::sleep(Duration::from_millis(300)).await;
    bridge.abort();

    let captured = events.lock().unwrap();
    assert_eq!(
        count_cron_events(&captured),
        1,
        "stable job set should emit exactly once (initial)"
    );
}

#[tokio::test]
async fn next_fire_changes_alone_do_not_re_emit() {
    let scheduler = Arc::new(CronScheduler::new());
    scheduler
        .add("*/5 * * * *", "job-a", true, false)
        .await
        .expect("add");
    let (frontend, events) = CaptureFrontend::new();
    let bridge = loopal_agent_server::testing::cron_bridge_spawn_with_interval(
        scheduler.clone(),
        Arc::new(frontend),
        TEST_INTERVAL,
    );

    // Multiple ticks pass; next_fire_unix_ms will differ each call, but the
    // bridge's diff ignores it and only emits once initially.
    tokio::time::sleep(Duration::from_millis(250)).await;
    bridge.abort();

    let captured = events.lock().unwrap();
    assert_eq!(count_cron_events(&captured), 1);
}
