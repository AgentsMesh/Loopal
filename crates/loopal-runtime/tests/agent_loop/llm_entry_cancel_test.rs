use std::sync::Arc;

use loopal_protocol::InterruptSignal;
use loopal_provider_api::StreamChunk;
use loopal_runtime::agent_loop::cancel::TurnCancel;

use super::{in_turn, make_runner_with_mock_provider};

#[tokio::test]
async fn pre_cancelled_stream_skips_the_provider() {
    let chunks = vec![Ok(StreamChunk::Text {
        text: "must-not-be-consumed".into(),
    })];
    let (mut runner, _event_rx, _input_tx, _ctrl_tx) = make_runner_with_mock_provider(chunks);
    let interrupt = InterruptSignal::new();
    interrupt.signal();
    let cancel = TurnCancel::new(interrupt, Arc::new(tokio::sync::watch::channel(0u64).0));

    let result = in_turn(runner.stream_llm_with(None, &cancel))
        .await
        .unwrap();
    assert!(result.stream_error);
    assert!(result.assistant_text.is_empty());
    assert!(result.tool_uses.is_empty());
}
