use std::sync::Arc;
use std::time::Duration;

use chrono::{TimeZone, Utc};
use loopal_protocol::{ControlCommand, Envelope, GateCloseReason, MessageSource};
use loopal_scheduler::{CronScheduler, ManualClock};
use loopal_test_support::{HarnessBuilder, chunks};
use serde_json::json;

use super::e2e_event_waiters::{wait_for_call_count, wait_for_gate_change};

#[tokio::test(start_paused = true)]
async fn suspend_blocks_scheduled_envelope_unsuspend_drains_pending() {
    let t0 = Utc.with_ymd_and_hms(2026, 5, 21, 10, 0, 30).unwrap();
    let clock = Arc::new(ManualClock::new(t0));
    let scheduler = Arc::new(CronScheduler::with_clock(clock.clone()));

    let calls = vec![
        chunks::tool_turn(
            "c1",
            "CronCreate",
            json!({"cron": "* * * * *", "prompt": "cron prompt"}),
        ),
        chunks::text_turn("scheduled the job"),
        chunks::text_turn("processed cron prompt after unsuspend"),
    ];

    let harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .lifecycle(loopal_runtime::LifecycleMode::Persistent)
        .scheduler(scheduler)
        .build()
        .await;
    let mut event_rx = harness.event_rx;
    let mailbox_tx = harness.mailbox_tx;
    let control_tx = harness.control_tx;
    let recorded = harness.recorded_messages;
    let mut runner = harness.runner;

    mailbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "kickoff"))
        .await
        .unwrap();
    let agent = tokio::spawn(async move { runner.run().await });

    wait_for_call_count(&recorded, 2, Duration::from_secs(5)).await;

    control_tx.send(ControlCommand::Suspend).await.unwrap();
    let closed = wait_for_gate_change(&mut event_rx, false).await;
    assert_eq!(closed.closed_reason, Some(GateCloseReason::UserSuspend));

    let baseline = recorded.lock().unwrap().len();
    clock.set(Utc.with_ymd_and_hms(2026, 5, 21, 10, 1, 5).unwrap());
    for _ in 0..10 {
        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    let after_advance = recorded.lock().unwrap().len();
    assert_eq!(
        after_advance, baseline,
        "Suspended must block cron envelope from reaching the LLM"
    );

    control_tx.send(ControlCommand::Unsuspend).await.unwrap();
    let _ = wait_for_gate_change(&mut event_rx, true).await;
    wait_for_call_count(&recorded, baseline + 1, Duration::from_secs(10)).await;

    drop(control_tx);
    drop(mailbox_tx);
    let _ = tokio::time::timeout(Duration::from_secs(5), agent).await;
}
