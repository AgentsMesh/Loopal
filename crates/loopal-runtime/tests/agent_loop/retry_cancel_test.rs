//! Tests for cancel-during-retry behavior in retry_stream_chat.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use loopal_error::{LoopalError, ProviderError};
use loopal_protocol::InterruptSignal;
use loopal_provider_api::{ChatParams, ChatStream, Provider, StopReason, StreamChunk};
use loopal_runtime::agent_loop::cancel::TurnCancel;

use super::mock_provider::{MockStreamChunks, make_runner_with_mock_provider};

/// Provider that fails N times with retryable 502 errors, then succeeds.
struct RetryableErrorProvider {
    failures: std::sync::Mutex<u32>,
}

impl RetryableErrorProvider {
    fn new(fail_count: u32) -> Self {
        Self {
            failures: std::sync::Mutex::new(fail_count),
        }
    }
}

#[async_trait::async_trait]
impl Provider for RetryableErrorProvider {
    fn name(&self) -> &str {
        "anthropic"
    }
    async fn stream_chat(&self, _p: &ChatParams) -> Result<ChatStream, LoopalError> {
        let should_fail = {
            let mut remaining = self.failures.lock().unwrap();
            if *remaining > 0 {
                *remaining -= 1;
                true
            } else {
                false
            }
        };
        if should_fail {
            // Simulate API latency so the cancel select! has time to fire
            tokio::time::sleep(Duration::from_millis(10)).await;
            Err(LoopalError::Provider(ProviderError::Api {
                status: 502,
                message: "Bad Gateway".into(),
            }))
        } else {
            let chunks = vec![
                Ok(StreamChunk::Text { text: "ok".into() }),
                Ok(StreamChunk::Usage {
                    input_tokens: 10,
                    output_tokens: 5,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                    thinking_tokens: 0,
                }),
                Ok(StreamChunk::Done {
                    stop_reason: StopReason::EndTurn,
                }),
            ];
            Ok(Box::pin(MockStreamChunks::new(VecDeque::from(chunks))))
        }
    }
}

/// Cancel during retry sleep interrupts the retry loop and returns empty stream.
#[tokio::test]
async fn test_cancel_during_retry_sleep() {
    // Use a simple mock provider that always returns 502
    let chunks = vec![Ok(StreamChunk::Text {
        text: "unused".into(),
    })];
    let (mut runner, mut event_rx, _mbox, _ctrl) = make_runner_with_mock_provider(chunks);

    // Replace the interrupt signal and watch channel so we can control them
    let interrupt = InterruptSignal::new();
    let tx = Arc::new(tokio::sync::watch::channel(0u64).0);
    let cancel = TurnCancel::new(interrupt.clone(), Arc::clone(&tx));

    // Register the retryable-error provider (always fails with 502)
    let kernel = Arc::get_mut(&mut runner.params.deps.kernel).unwrap();
    kernel.register_provider(Arc::new(RetryableErrorProvider::new(10)) as Arc<dyn Provider>);

    // Drain events in background
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let params = runner
        .prepare_chat_params_with(runner.params.store.messages())
        .unwrap();
    let provider = runner
        .params
        .deps
        .kernel
        .resolve_provider(&runner.params.config.model)
        .unwrap();

    // Signal cancel after a short delay (during retry sleep)
    let tx2 = Arc::clone(&tx);
    tokio::spawn(async move {
        // Wait enough for the first API call + error event, but before retry sleep ends
        tokio::time::sleep(Duration::from_millis(100)).await;
        interrupt.signal();
        tx2.send_modify(|v| *v = v.wrapping_add(1));
    });

    // retry_stream_chat should exit early due to cancel
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        runner.retry_stream_chat(&params, &*provider, &cancel),
    )
    .await;

    let stream = result
        .expect("should not timeout")
        .expect("should not error");
    // The returned stream should be empty (cancelled)
    use futures::StreamExt;
    let items: Vec<_> = stream.collect().await;
    assert!(
        items.is_empty(),
        "cancelled retry should return empty stream"
    );
}

/// Cancel via is_cancelled() check at loop top before stream_chat.
#[tokio::test]
async fn test_cancel_before_stream_chat_attempt() {
    let chunks = vec![Ok(StreamChunk::Text {
        text: "unused".into(),
    })];
    let (mut runner, mut event_rx, _mbox, _ctrl) = make_runner_with_mock_provider(chunks);

    // Pre-signal the interrupt before calling retry_stream_chat
    let interrupt = InterruptSignal::new();
    interrupt.signal();
    let tx = Arc::new(tokio::sync::watch::channel(0u64).0);
    let cancel = TurnCancel::new(interrupt, tx);

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let params = runner
        .prepare_chat_params_with(runner.params.store.messages())
        .unwrap();
    let provider = runner
        .params
        .deps
        .kernel
        .resolve_provider(&runner.params.config.model)
        .unwrap();

    let stream = runner
        .retry_stream_chat(&params, &*provider, &cancel)
        .await
        .expect("should not error");

    use futures::StreamExt;
    let items: Vec<_> = stream.collect().await;
    assert!(items.is_empty(), "pre-cancelled should return empty stream");
}
