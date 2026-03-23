//! Unit tests for TurnCancel: the per-turn cancellation scope.

use std::sync::Arc;
use std::time::Duration;

use loopal_protocol::InterruptSignal;
use loopal_runtime::agent_loop::cancel::TurnCancel;

/// cancelled() returns immediately when interrupt is already signaled.
#[tokio::test]
async fn test_cancelled_returns_immediately_when_pre_signaled() {
    let interrupt = InterruptSignal::new();
    let tx = Arc::new(tokio::sync::watch::channel(0u64).0);
    interrupt.signal();

    let cancel = TurnCancel::new(interrupt, tx);
    // Must return within a short timeout — not hang
    tokio::time::timeout(Duration::from_millis(50), cancel.cancelled())
        .await
        .expect("cancelled() should return immediately for pre-signaled interrupt");
    assert!(cancel.is_cancelled());
}

/// cancelled() wakes up when signaled from another task.
#[tokio::test]
async fn test_cancelled_wakes_on_signal_from_another_task() {
    let interrupt = InterruptSignal::new();
    let tx = Arc::new(tokio::sync::watch::channel(0u64).0);
    let cancel = TurnCancel::new(interrupt.clone(), Arc::clone(&tx));

    // Signal after a short delay from another task
    let tx2 = Arc::clone(&tx);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        interrupt.signal();
        tx2.send_modify(|v| *v = v.wrapping_add(1));
    });

    tokio::time::timeout(Duration::from_millis(200), cancel.cancelled())
        .await
        .expect("cancelled() should wake when signaled");
    assert!(cancel.is_cancelled());
}

/// cancelled() returns when the watch sender is dropped.
#[tokio::test]
async fn test_cancelled_returns_on_sender_drop() {
    let interrupt = InterruptSignal::new();
    let tx = Arc::new(tokio::sync::watch::channel(0u64).0);
    let cancel = TurnCancel::new(interrupt, Arc::clone(&tx));

    // Drop the external reference — TurnCancel still holds one via _interrupt_tx
    drop(tx);

    // cancelled() should NOT return because _interrupt_tx keeps sender alive.
    // Verify it does NOT return within 50ms.
    let result = tokio::time::timeout(Duration::from_millis(50), cancel.cancelled()).await;
    assert!(
        result.is_err(),
        "cancelled() should hang when sender is alive and not signaled"
    );
}

/// is_cancelled() bridges InterruptSignal to CancellationToken.
#[tokio::test]
async fn test_is_cancelled_bridges_signal_to_token() {
    let interrupt = InterruptSignal::new();
    let tx = Arc::new(tokio::sync::watch::channel(0u64).0);
    let cancel = TurnCancel::new(interrupt.clone(), tx);

    assert!(!cancel.is_cancelled());
    interrupt.signal();
    assert!(cancel.is_cancelled());
    // Subsequent calls also return true (token stays cancelled)
    assert!(cancel.is_cancelled());
}

/// cancelled() does not hang in select! when used with another future.
#[tokio::test]
async fn test_cancelled_in_select_with_sleep() {
    let interrupt = InterruptSignal::new();
    let tx = Arc::new(tokio::sync::watch::channel(0u64).0);
    let cancel = TurnCancel::new(interrupt.clone(), Arc::clone(&tx));

    let tx2 = Arc::clone(&tx);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        interrupt.signal();
        tx2.send_modify(|v| *v = v.wrapping_add(1));
    });

    // Simulate the retry_stream_chat pattern
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(10)) => {
            panic!("sleep should not complete — cancel should fire first");
        }
        _ = cancel.cancelled() => {
            // Expected: cancel fires before the 10s sleep
        }
    }
    assert!(cancel.is_cancelled());
}
