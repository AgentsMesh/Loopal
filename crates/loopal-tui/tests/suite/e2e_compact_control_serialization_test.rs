use loopal_protocol::{AgentEventPayload, ControlCommand, Envelope, MessageSource};
use loopal_test_support::{HarnessBuilder, SpawnedHarness, scenarios};
use loopal_tui::app::App;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::e2e_harness::TuiTestHarness;

fn wrap_tui(inner: SpawnedHarness) -> TuiTestHarness {
    let terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let app = App::new(
        inner.session_ctrl.clone(),
        inner.fixture.path().to_path_buf(),
    );
    TuiTestHarness {
        terminal,
        app,
        inner,
    }
}

#[tokio::test]
async fn compact_control_during_streaming_does_not_interleave_turn() {
    // Control channel must be serialized against the turn loop: a
    // `Compact` command queued while the LLM is mid-stream must wait
    // until the streaming turn completes — never interleave Compacted
    // events between Stream/TokenUsage events of the in-flight turn.
    //
    // Send a `Human` envelope to start the turn, then immediately queue
    // Compact before draining. Two turn-worth of LLM calls let the
    // second one service compaction.
    let calls = scenarios::two_turn("first turn streaming...", "compact summary");
    let inner = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .build_spawned()
        .await;
    let mut harness = wrap_tui(inner);

    let _ = harness.collect_until_idle().await;

    harness
        .inner
        .mailbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "go"))
        .await
        .unwrap();
    harness
        .inner
        .control_tx
        .send(ControlCommand::Compact { instructions: None })
        .await
        .unwrap();

    let evts = harness.collect_until_idle().await;

    let mut first_stream_idx = None;
    let mut compacted_idx = None;
    for (i, e) in evts.iter().enumerate() {
        match e {
            AgentEventPayload::Stream { text }
                if first_stream_idx.is_none() && text.contains("first turn streaming") =>
            {
                first_stream_idx = Some(i);
            }
            AgentEventPayload::Compacted(_) if compacted_idx.is_none() => {
                compacted_idx = Some(i);
            }
            _ => {}
        }
    }

    let stream_pos =
        first_stream_idx.expect("turn-1 stream text must appear in the event sequence");
    if let Some(compact_pos) = compacted_idx {
        assert!(
            stream_pos < compact_pos,
            "turn streaming must finish before Compacted event fires; \
             got stream@{stream_pos} compacted@{compact_pos}",
        );
    }
}
