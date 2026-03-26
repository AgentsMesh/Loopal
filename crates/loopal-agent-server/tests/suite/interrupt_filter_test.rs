//! Unit and integration tests for the interrupt_filter module.
//!
//! Unit tests use raw `mpsc::channel` to inject `Incoming` messages directly.
//! The integration test verifies the full chain: filter → InterruptSignal +
//! watch → TurnCancel::cancelled() wakes up — proving the fix works end-to-end.

use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_protocol::InterruptSignal;
use tokio::sync::{mpsc, watch};

fn make_filter() -> (
    mpsc::Sender<Incoming>,
    mpsc::Receiver<Incoming>,
    InterruptSignal,
    watch::Receiver<u64>,
) {
    let (raw_tx, raw_rx) = mpsc::channel::<Incoming>(16);
    let interrupt = InterruptSignal::new();
    let (watch_tx, watch_rx) = watch::channel(0u64);
    let filtered_rx =
        loopal_agent_server::interrupt_filter::spawn(raw_rx, interrupt.clone(), Arc::new(watch_tx));
    (raw_tx, filtered_rx, interrupt, watch_rx)
}

#[tokio::test]
async fn filter_signals_and_wakes_watch_on_interrupt() {
    let (raw_tx, _filtered_rx, interrupt, mut watch_rx) = make_filter();

    raw_tx
        .send(Incoming::Notification {
            method: methods::AGENT_INTERRUPT.name.to_string(),
            params: serde_json::Value::Null,
        })
        .await
        .unwrap();

    let result = tokio::time::timeout(Duration::from_secs(2), watch_rx.changed()).await;
    assert!(result.is_ok(), "watch should be notified");
    assert!(*watch_rx.borrow() > 0);
    assert!(interrupt.is_signaled());
}

#[tokio::test]
async fn filter_forwards_non_interrupt_request() {
    let (raw_tx, mut filtered_rx, _interrupt, _watch_rx) = make_filter();

    raw_tx
        .send(Incoming::Request {
            id: 42,
            method: methods::AGENT_MESSAGE.name.to_string(),
            params: serde_json::json!({"hello": true}),
        })
        .await
        .unwrap();

    let msg = tokio::time::timeout(Duration::from_secs(2), filtered_rx.recv())
        .await
        .unwrap()
        .unwrap();
    match msg {
        Incoming::Request { id, method, .. } => {
            assert_eq!(id, 42);
            assert_eq!(method, methods::AGENT_MESSAGE.name);
        }
        _ => panic!("expected request to be forwarded"),
    }
}

#[tokio::test]
async fn filter_forwards_non_interrupt_notification() {
    let (raw_tx, mut filtered_rx, _interrupt, _watch_rx) = make_filter();

    raw_tx
        .send(Incoming::Notification {
            method: methods::AGENT_EVENT.name.to_string(),
            params: serde_json::json!({}),
        })
        .await
        .unwrap();

    let msg = tokio::time::timeout(Duration::from_secs(2), filtered_rx.recv())
        .await
        .unwrap()
        .unwrap();
    match msg {
        Incoming::Notification { method, .. } => {
            assert_eq!(method, methods::AGENT_EVENT.name);
        }
        _ => panic!("expected non-interrupt notification to be forwarded"),
    }
}

#[tokio::test]
async fn filter_does_not_forward_interrupt() {
    let (raw_tx, mut filtered_rx, _interrupt, _watch_rx) = make_filter();

    // Send interrupt then a normal request
    raw_tx
        .send(Incoming::Notification {
            method: methods::AGENT_INTERRUPT.name.to_string(),
            params: serde_json::Value::Null,
        })
        .await
        .unwrap();
    raw_tx
        .send(Incoming::Request {
            id: 1,
            method: methods::AGENT_MESSAGE.name.to_string(),
            params: serde_json::json!({}),
        })
        .await
        .unwrap();

    // First message on filtered_rx should be the request, not the interrupt
    let msg = tokio::time::timeout(Duration::from_secs(2), filtered_rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(msg, Incoming::Request { id: 1, .. }),
        "interrupt should be consumed, request should appear first"
    );
}

#[tokio::test]
async fn filter_handles_multiple_interrupts() {
    let (raw_tx, _filtered_rx, interrupt, mut watch_rx) = make_filter();

    for _ in 0..3 {
        raw_tx
            .send(Incoming::Notification {
                method: methods::AGENT_INTERRUPT.name.to_string(),
                params: serde_json::Value::Null,
            })
            .await
            .unwrap();
    }

    // Wait for all to be processed
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(interrupt.is_signaled());

    // watch value should have been incremented (at least once observable)
    let _ = tokio::time::timeout(Duration::from_secs(1), watch_rx.changed()).await;
    assert!(*watch_rx.borrow() >= 1);
}

// ── Integration: filter → TurnCancel end-to-end ─────────────────────

/// Proves the full cancellation chain works: an IPC interrupt notification
/// flowing through the filter wakes `TurnCancel::cancelled()` — exactly
/// the path that was broken before this fix.
#[tokio::test]
async fn filter_wakes_turn_cancel_on_interrupt() {
    use loopal_runtime::agent_loop::cancel::TurnCancel;

    let (raw_tx, raw_rx) = mpsc::channel::<Incoming>(16);
    let interrupt = InterruptSignal::new();
    let (watch_tx, _watch_rx) = watch::channel(0u64);
    let interrupt_tx = Arc::new(watch_tx);

    // Wire filter with the SAME interrupt + watch that TurnCancel will use
    let _filtered_rx = loopal_agent_server::interrupt_filter::spawn(
        raw_rx,
        interrupt.clone(),
        interrupt_tx.clone(),
    );
    let cancel = TurnCancel::new(interrupt.clone(), interrupt_tx.clone());

    // Simulate: TUI ESC → forward_interrupt → IPC notification arrives
    raw_tx
        .send(Incoming::Notification {
            method: methods::AGENT_INTERRUPT.name.to_string(),
            params: serde_json::Value::Null,
        })
        .await
        .unwrap();

    // TurnCancel::cancelled() must wake up (this was the broken path)
    let result = tokio::time::timeout(Duration::from_secs(2), cancel.cancelled()).await;
    assert!(
        result.is_ok(),
        "TurnCancel::cancelled() should wake up when interrupt filter processes notification"
    );
    assert!(cancel.is_cancelled());
}
