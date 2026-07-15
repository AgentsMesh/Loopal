use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_provider_api::{StopReason, StreamChunk};

use super::try_recover_helpers::{
    Outcome, context_overflow_err, make_runner, ok_done, seed_prior_completed_turn,
};

#[tokio::test]
async fn context_overflow_recovery_emits_compacting_banner() {
    let truncated_with_tools = vec![
        Ok(StreamChunk::Text {
            text: "partial".into(),
        }),
        Ok(StreamChunk::ToolUse {
            id: "tc-1".into(),
            name: "Read".into(),
            input: serde_json::json!({"file_path": "/tmp/x"}),
        }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::MaxTokens,
        }),
    ];
    let (mut runner, _calls, mut rx) = make_runner(vec![
        Outcome::Stream(truncated_with_tools),
        Outcome::Err(context_overflow_err()),
        Outcome::Stream(ok_done()),
        Outcome::Stream(ok_done()),
    ]);
    seed_prior_completed_turn(&mut runner);

    let collected: std::sync::Arc<std::sync::Mutex<Vec<AgentEvent>>> = Default::default();
    let collector = collected.clone();
    let collector_handle = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            collector.lock().unwrap().push(event);
        }
    });

    let _ = runner.run().await.unwrap();
    drop(runner);
    collector_handle.await.expect("collector task panicked");

    let events = collected.lock().unwrap().clone();
    let saw_banner = events.iter().any(|event| {
        matches!(
            &event.payload,
            AgentEventPayload::ProviderWarning { message }
                if message == loopal_runtime::agent_loop::CONTEXT_OVERFLOW_BANNER
        )
    });
    assert!(
        saw_banner,
        "ContextOverflow recovery must emit exact CONTEXT_OVERFLOW_BANNER; got events: {events:?}"
    );
}
