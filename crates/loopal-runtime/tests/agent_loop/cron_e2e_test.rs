//! End-to-end cron scheduler integration test.
//!
//! Verifies the full pipeline: LLM calls CronCreate → scheduler stores task
//! → ManualClock advances → tick_loop fires → adapter converts to Envelope
//! → wait_for_input select picks up → LLM processes scheduled prompt.

use std::sync::Arc;
use std::time::Duration;

use chrono::{TimeZone, Utc};
use serde_json::json;

use loopal_protocol::AgentEventPayload;
use loopal_scheduler::{CronScheduler, ManualClock};
use loopal_test_support::{HarnessBuilder, chunks};

/// Full lifecycle: CronCreate → clock advance → trigger fires → LLM responds.
#[tokio::test(start_paused = true)]
async fn cron_create_then_trigger_fires() {
    // Start at 10:00:30 — next "* * * * *" fires at 10:01:00.
    let t0 = Utc.with_ymd_and_hms(2026, 3, 29, 10, 0, 30).unwrap();
    let clock = Arc::new(ManualClock::new(t0));
    let scheduler = Arc::new(CronScheduler::with_clock(clock.clone()));

    // LLM turn 1: call CronCreate tool.
    // LLM turn 2: confirm creation with text.
    // LLM turn 3: respond to the scheduled prompt (after trigger fires).
    let calls = vec![
        chunks::tool_turn(
            "tc1",
            "CronCreate",
            json!({"cron": "* * * * *", "prompt": "check deploys"}),
        ),
        chunks::text_turn("Scheduled a cron job."),
        chunks::text_turn("Checked deploys — all good."),
    ];

    let harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .max_turns(5)
        .scheduler(scheduler)
        .build()
        .await;

    let mut event_rx = harness.event_rx;
    let mailbox_tx = harness.mailbox_tx;
    let mut runner = harness.runner;

    // Send initial message via mailbox (store starts empty → no pending prompt)
    mailbox_tx
        .send(loopal_protocol::Envelope::new(
            loopal_protocol::MessageSource::Human,
            "main",
            "start",
        ))
        .await
        .unwrap();

    let agent_handle = tokio::spawn(async move { runner.run().await });

    // Wait for the agent to finish turn 1+2 and become idle.
    let mut awaiting_count = 0;
    loop {
        let event = tokio::time::timeout(Duration::from_secs(10), event_rx.recv())
            .await
            .expect("event timeout")
            .expect("event channel");
        if matches!(event.payload, AgentEventPayload::AwaitingInput) {
            awaiting_count += 1;
            if awaiting_count == 1 {
                // Agent idle after CronCreate + text. Advance clock to trigger.
                clock.set(Utc.with_ymd_and_hms(2026, 3, 29, 10, 1, 5).unwrap());
                for _ in 0..5 {
                    tokio::time::advance(Duration::from_secs(1)).await;
                    tokio::task::yield_now().await;
                }
                continue;
            }
            if awaiting_count == 2 {
                // Agent processed the scheduled prompt (turn 3). Done.
                drop(mailbox_tx);
                break;
            }
        }
    }

    drop(event_rx);
    let result = tokio::time::timeout(Duration::from_secs(10), agent_handle)
        .await
        .expect("agent should finish")
        .expect("task should not panic");
    assert!(result.is_ok());
}
