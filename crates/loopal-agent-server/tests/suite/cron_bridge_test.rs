//! Basic tests for cron_bridge — scheduler → CronsChanged emit happy paths.

use std::sync::Arc;
use std::time::Duration;

use loopal_protocol::AgentEventPayload;
use loopal_scheduler::CronScheduler;

use super::cron_bridge_helpers::{
    CaptureFrontend, TEST_INTERVAL, count_cron_events, last_cron_ids,
};

#[tokio::test]
async fn durable_flag_propagates_to_snapshot() {
    let scheduler = Arc::new(CronScheduler::new());
    // durable=true on an in-memory scheduler still tags the task; the
    // CronJobInfo → CronJobSnapshot conversion must carry it through
    // so the TUI can distinguish persisted tasks from transient ones.
    scheduler
        .add("*/5 * * * *", "persistent", true, true)
        .await
        .expect("add");
    let (frontend, events) = CaptureFrontend::new();
    let bridge = loopal_agent_server::testing::cron_bridge_spawn_with_interval(
        scheduler.clone(),
        Arc::new(frontend),
        TEST_INTERVAL,
    );

    tokio::time::sleep(Duration::from_millis(150)).await;
    bridge.abort();

    let captured = events.lock().unwrap();
    let durable_flag = captured
        .iter()
        .rev()
        .find_map(|e| match e {
            AgentEventPayload::CronsChanged { crons } if !crons.is_empty() => {
                Some(crons[0].durable)
            }
            _ => None,
        })
        .expect("should have a non-empty CronsChanged");
    assert!(durable_flag, "CronJobSnapshot.durable must be true");
}

#[tokio::test]
async fn emits_initial_empty_snapshot() {
    let scheduler = Arc::new(CronScheduler::new());
    let (frontend, events) = CaptureFrontend::new();
    let bridge = loopal_agent_server::testing::cron_bridge_spawn_with_interval(
        scheduler.clone(),
        Arc::new(frontend),
        TEST_INTERVAL,
    );

    tokio::time::sleep(Duration::from_millis(80)).await;
    bridge.abort();

    let captured = events.lock().unwrap();
    assert!(count_cron_events(&captured) >= 1);
    if let Some(ids) = last_cron_ids(&captured) {
        assert!(ids.is_empty());
    } else {
        panic!("expected CronsChanged");
    }
}

#[tokio::test]
async fn emits_after_scheduler_add() {
    let scheduler = Arc::new(CronScheduler::new());
    let (frontend, events) = CaptureFrontend::new();
    let bridge = loopal_agent_server::testing::cron_bridge_spawn_with_interval(
        scheduler.clone(),
        Arc::new(frontend),
        TEST_INTERVAL,
    );

    tokio::time::sleep(Duration::from_millis(80)).await;
    let id = scheduler
        .add("*/5 * * * *", "say hello", true, false)
        .await
        .expect("add");
    tokio::time::sleep(Duration::from_millis(200)).await;
    bridge.abort();

    let captured = events.lock().unwrap();
    let ids = last_cron_ids(&captured).expect("should have CronsChanged");
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0], id);
}

#[tokio::test]
async fn emits_after_scheduler_remove() {
    let scheduler = Arc::new(CronScheduler::new());
    let id = scheduler
        .add("*/5 * * * *", "temp task", true, false)
        .await
        .expect("add");
    let (frontend, events) = CaptureFrontend::new();
    let bridge = loopal_agent_server::testing::cron_bridge_spawn_with_interval(
        scheduler.clone(),
        Arc::new(frontend),
        TEST_INTERVAL,
    );

    tokio::time::sleep(Duration::from_millis(80)).await;
    assert!(scheduler.remove(&id).await);
    tokio::time::sleep(Duration::from_millis(200)).await;
    bridge.abort();

    let captured = events.lock().unwrap();
    let ids = last_cron_ids(&captured).expect("should have CronsChanged");
    assert!(ids.is_empty());
}

#[tokio::test]
async fn prompt_newlines_are_normalized() {
    let scheduler = Arc::new(CronScheduler::new());
    scheduler
        .add("*/5 * * * *", "line1\nline2\rline3", true, false)
        .await
        .expect("add");
    let (frontend, events) = CaptureFrontend::new();
    let bridge = loopal_agent_server::testing::cron_bridge_spawn_with_interval(
        scheduler.clone(),
        Arc::new(frontend),
        TEST_INTERVAL,
    );

    tokio::time::sleep(Duration::from_millis(150)).await;
    bridge.abort();

    let captured = events.lock().unwrap();
    let prompt = captured
        .iter()
        .rev()
        .find_map(|e| match e {
            AgentEventPayload::CronsChanged { crons } if !crons.is_empty() => {
                Some(crons[0].prompt.clone())
            }
            _ => None,
        })
        .expect("should have cron");
    assert!(!prompt.contains('\n'));
    assert!(!prompt.contains('\r'));
    assert!(prompt.contains("line1 line2line3"));
}
