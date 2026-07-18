//! Integration test: a stalled LLM stream is bounded by the idle timeout and
//! recovers through the existing truncation → auto-continue path, instead of
//! blocking on the provider's 300s total-request cap.

use std::sync::Arc;
use std::time::Duration;

use loopal_provider_api::{StopReason, StreamChunk};
use loopal_test_support::mock_provider::StallThenProvider;

use super::mock_provider::make_runner_with_dyn_provider;

#[tokio::test(start_paused = true)]
async fn stalled_stream_recovers_via_idle_timeout() {
    // First call streams a little text then goes silent for far longer than the
    // idle timeout (Done is never reached); the auto-continue call then finishes.
    let provider = Arc::new(StallThenProvider::new(
        vec![
            Ok(StreamChunk::Text {
                text: "partial".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
        vec![vec![
            Ok(StreamChunk::Text {
                text: "Recovered.".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ]],
        Duration::from_secs(600),
    ));

    let (mut runner, mut event_rx) = make_runner_with_dyn_provider(provider);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();

    assert_eq!(
        output.result, "Recovered.",
        "a stalled stream should auto-continue and yield the recovery text"
    );
    assert!(
        runner.turn_count >= 1,
        "the turn should have completed after recovery"
    );
}
