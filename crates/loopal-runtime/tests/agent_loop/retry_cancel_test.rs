//! Tests for cancel-during-retry behavior in `retry_stream_response`.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use loopal_error::{LoopalError, ProviderError};
use loopal_protocol::{AgentEventPayload, InterruptSignal};
use loopal_provider_api::{ChatParams, ChatStream, Provider, StopReason, StreamChunk};
use loopal_runtime::agent_loop::cancel::TurnCancel;

use super::in_turn;
use super::mock_provider::{
    MockStreamChunks, make_multi_runner_with_intents, make_runner_with_mock_provider,
};

fn retryable_stream_error(message: &str) -> Result<StreamChunk, LoopalError> {
    Err(LoopalError::Provider(ProviderError::Api {
        status: 502,
        message: message.into(),
        retry_after_ms: Some(1),
    }))
}

/// Provider that fails N times with retryable 502 errors, then succeeds.
struct RetryableErrorProvider {
    failures: std::sync::Mutex<u32>,
}

struct RetryThenFatalProvider {
    calls: std::sync::atomic::AtomicU32,
}

struct PendingHandshakeProvider;

#[async_trait::async_trait]
impl Provider for PendingHandshakeProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn stream_chat(&self, _p: &ChatParams) -> Result<ChatStream, LoopalError> {
        std::future::pending().await
    }
}

#[async_trait::async_trait]
impl Provider for RetryThenFatalProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn stream_chat(&self, _p: &ChatParams) -> Result<ChatStream, LoopalError> {
        let call = self
            .calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if call == 0 {
            Err(LoopalError::Provider(ProviderError::RateLimited {
                retry_after_ms: 1,
            }))
        } else {
            Err(LoopalError::Provider(ProviderError::Api {
                status: 400,
                message: "fatal".into(),
                retry_after_ms: None,
            }))
        }
    }
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
                retry_after_ms: None,
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

    let params = runner.prepare_chat_params(None).unwrap();
    let provider = runner
        .params
        .deps
        .kernel
        .resolve_provider(&runner.params.config.model())
        .unwrap();

    // Signal cancel after a short delay (during retry sleep)
    let tx2 = Arc::clone(&tx);
    tokio::spawn(async move {
        // Wait enough for the first API call + error event, but before retry sleep ends
        tokio::time::sleep(Duration::from_millis(100)).await;
        interrupt.signal();
        tx2.send_modify(|v| *v = v.wrapping_add(1));
    });

    // The complete-attempt retry loop should exit early due to cancel.
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        in_turn(runner.retry_stream_response(&params, &*provider, &cancel)),
    )
    .await;

    let response = result
        .expect("should not timeout")
        .expect("should not error");
    assert!(response.stream_error);
    assert!(response.assistant_text.is_empty());
    let retry_events: Vec<_> = std::iter::from_fn(|| event_rx.try_recv().ok())
        .filter_map(|event| match event.payload {
            AgentEventPayload::RetryError { .. } => Some("retry"),
            AgentEventPayload::RetryCleared => Some("cleared"),
            _ => None,
        })
        .collect();
    assert_eq!(retry_events, vec!["retry", "cleared"]);
}

/// Cancel via is_cancelled() check at loop top before stream_chat.
#[tokio::test]
async fn test_cancel_before_stream_chat_attempt() {
    let chunks = vec![Ok(StreamChunk::Text {
        text: "unused".into(),
    })];
    let (mut runner, mut event_rx, _mbox, _ctrl) = make_runner_with_mock_provider(chunks);

    // Pre-signal the interrupt before starting a provider attempt.
    let interrupt = InterruptSignal::new();
    interrupt.signal();
    let tx = Arc::new(tokio::sync::watch::channel(0u64).0);
    let cancel = TurnCancel::new(interrupt, tx);

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let params = runner.prepare_chat_params(None).unwrap();
    let provider = runner
        .params
        .deps
        .kernel
        .resolve_provider(&runner.params.config.model())
        .unwrap();

    let response = in_turn(runner.retry_stream_response(&params, &*provider, &cancel))
        .await
        .expect("should not error");

    assert!(response.stream_error);
    assert!(response.assistant_text.is_empty());
}

#[tokio::test]
async fn cancel_interrupts_pending_provider_handshake() {
    let chunks = vec![Ok(StreamChunk::Text {
        text: "unused".into(),
    })];
    let (mut runner, _event_rx, _mbox, _ctrl) = make_runner_with_mock_provider(chunks);
    let kernel = Arc::get_mut(&mut runner.params.deps.kernel).unwrap();
    kernel.register_provider(Arc::new(PendingHandshakeProvider) as Arc<dyn Provider>);

    let params = runner.prepare_chat_params(None).unwrap();
    let provider = runner
        .params
        .deps
        .kernel
        .resolve_provider(&runner.params.config.model())
        .unwrap();
    let interrupt = InterruptSignal::new();
    let tx = Arc::new(tokio::sync::watch::channel(0u64).0);
    let cancel = TurnCancel::new(interrupt.clone(), Arc::clone(&tx));
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        interrupt.signal();
        tx.send_modify(|generation| *generation = generation.wrapping_add(1));
    });

    let response = tokio::time::timeout(
        Duration::from_millis(500),
        in_turn(runner.retry_stream_response(&params, &*provider, &cancel)),
    )
    .await
    .expect("cancellation must abort the provider handshake")
    .expect("cancellation returns an interrupted response");

    assert!(response.stream_error);
    assert!(response.assistant_text.is_empty());
}

#[tokio::test]
async fn terminal_error_clears_retry_state() {
    let chunks = vec![Ok(StreamChunk::Text {
        text: "unused".into(),
    })];
    let (mut runner, mut event_rx, _mbox, _ctrl) = make_runner_with_mock_provider(chunks);

    let kernel = Arc::get_mut(&mut runner.params.deps.kernel).unwrap();
    kernel.register_provider(Arc::new(RetryThenFatalProvider {
        calls: std::sync::atomic::AtomicU32::new(0),
    }) as Arc<dyn Provider>);

    let params = runner.prepare_chat_params(None).unwrap();
    let provider = runner
        .params
        .deps
        .kernel
        .resolve_provider(&runner.params.config.model())
        .unwrap();
    let cancel = TurnCancel::new(
        InterruptSignal::new(),
        Arc::new(tokio::sync::watch::channel(0u64).0),
    );

    let result = in_turn(runner.retry_stream_response(&params, &*provider, &cancel)).await;
    assert!(result.is_err());

    let retry_events: Vec<_> = std::iter::from_fn(|| event_rx.try_recv().ok())
        .filter_map(|event| match event.payload {
            AgentEventPayload::RetryError { .. } => Some("retry"),
            AgentEventPayload::RetryCleared => Some("cleared"),
            _ => None,
        })
        .collect();
    assert_eq!(retry_events, vec!["retry", "cleared"]);
}

#[tokio::test(start_paused = true)]
async fn retryable_stream_error_before_output_replays_same_request() {
    let calls = vec![
        vec![retryable_stream_error("gateway reset")],
        vec![
            Ok(StreamChunk::Text {
                text: "recovered".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let (mut runner, mut event_rx, intents) = make_multi_runner_with_intents(calls);
    let cancel = super::make_cancel();

    let result = in_turn(runner.stream_llm_with(None, &cancel))
        .await
        .unwrap();
    assert_eq!(result.assistant_text, "recovered");
    assert_eq!(intents.lock().unwrap().as_slice(), &[None, None]);

    let lifecycle: Vec<_> = std::iter::from_fn(|| event_rx.try_recv().ok())
        .filter_map(|event| match event.payload {
            AgentEventPayload::RetryError { .. } => Some("retry"),
            AgentEventPayload::RetryCleared => Some("cleared"),
            _ => None,
        })
        .collect();
    assert_eq!(lifecycle, ["retry", "cleared"]);
}

#[tokio::test(start_paused = true)]
async fn retryable_stream_error_exhaustion_is_terminal_and_balanced() {
    let calls = (0..=6)
        .map(|attempt| vec![retryable_stream_error(&format!("attempt {attempt}"))])
        .collect();
    let (mut runner, mut event_rx, intents) = make_multi_runner_with_intents(calls);
    let cancel = super::make_cancel();

    let error = in_turn(runner.stream_llm_with(None, &cancel))
        .await
        .expect_err("seven failed attempts must exhaust six retries");
    assert!(error.to_string().contains("attempt 6"));
    assert_eq!(intents.lock().unwrap().len(), 7);

    let lifecycle: Vec<_> = std::iter::from_fn(|| event_rx.try_recv().ok())
        .filter_map(|event| match event.payload {
            AgentEventPayload::RetryError { attempt, .. } => Some(format!("retry:{attempt}")),
            AgentEventPayload::RetryCleared => Some("cleared".into()),
            _ => None,
        })
        .collect();
    assert_eq!(
        lifecycle,
        [
            "retry:1", "retry:2", "retry:3", "retry:4", "retry:5", "retry:6", "cleared"
        ]
    );
}
