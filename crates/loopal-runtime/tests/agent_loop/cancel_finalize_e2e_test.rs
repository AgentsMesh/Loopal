use std::time::Duration;

use loopal_protocol::{AgentEventPayload, Envelope, MessageSource};
use loopal_runtime::agent_loop::WaitResult;
use loopal_test_support::{HarnessBuilder, TestFixture, chunks};
use loopal_turn::{
    AssistantOutput, OrderedToolBatch, StopReason, ToolBatchItem, ToolCall, ToolCallId,
    ToolExecState, Turn, TurnStep, TurnTrigger,
};

use super::e2e_event_waiters::{wait_for_interrupted_event, wait_for_stream_event};
use super::goal_e2e_test::make_goal_session;

fn in_progress_turn_with_open_tool() -> Turn {
    let call = ToolCall {
        id: ToolCallId::new("t1"),
        name: "Bash".into(),
        input: serde_json::Value::Null,
    };
    let mut turn = Turn::new(TurnTrigger::UserInput {
        envelope_id: "e1".into(),
        content: "do work".into(),
        images: vec![],
    });
    turn.body.steps = vec![
        TurnStep::LlmCall {
            model: "m".into(),
            response: AssistantOutput {
                text_blocks: vec![],
                tool_calls: vec![call.clone()],
                server_blocks: vec![],
                stop_reason: StopReason::ToolUse,
            },
        },
        TurnStep::ToolBatch(OrderedToolBatch {
            items: vec![ToolBatchItem {
                call,
                state: ToolExecState::Pending,
            }],
        }),
    ];
    turn
}

// Verifies the ingest→finalize path: ingesting a new envelope while a turn is
// still in-progress emits an observable TurnCancelled and resets continuation
// state (vs the old silent end_turn_record). The pending→Cancelled pairing is
// NOT exercised here (seed_test_turns leaves the tool batch unopened); that is
// covered by the cancel_open_tool_batch_marks_all_pending_as_cancelled unit test.
#[tokio::test]
async fn ingest_while_turn_in_progress_emits_turn_cancelled() {
    let inner = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("x")])
        .messages(vec![])
        .lifecycle(loopal_runtime::LifecycleMode::Persistent)
        .build()
        .await;
    let mut runner = inner.runner;
    let mut event_rx = inner.event_rx;

    runner.seed_test_turns(vec![in_progress_turn_with_open_tool()]);
    runner.last_continuation_goal_id = Some("stale-goal".into());

    inner
        .mailbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "next"))
        .await
        .unwrap();

    let r = tokio::time::timeout(Duration::from_secs(2), runner.wait_for_input())
        .await
        .expect("wait_for_input must not hang")
        .unwrap();
    assert!(matches!(r, Some(WaitResult::MessageAdded)));

    // finalize must have reset the stale continuation goal.
    assert!(
        runner.last_continuation_goal_id.is_none(),
        "cancellation must reset continuation state"
    );

    let mut saw_cancelled = false;
    while let Ok(ev) = event_rx.try_recv() {
        if let AgentEventPayload::TurnCancelled { cause } = &ev.payload {
            assert_eq!(cause, "ParentTurnAborted");
            saw_cancelled = true;
        }
    }
    assert!(
        saw_cancelled,
        "in-progress turn cancelled by new envelope must emit TurnCancelled"
    );

    drop(inner.mailbox_tx);
    drop(inner.control_tx);
}

// Interrupt mid-stream must finalize the turn: emit TurnCancelled (after the
// Interrupted event) so the interrupted turn is collected as
// Cancelled{UserInterrupt}, not left InProgress to be mislabeled later.
#[tokio::test]
async fn interrupt_mid_stream_finalizes_turn_as_cancelled() {
    let fixture = TestFixture::new();
    let (_tmp, session, _log) = make_goal_session(&fixture.test_session("interrupt-finalize").id);
    session.create("ongoing".into()).await.unwrap();
    let mut harness = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("streaming...")])
        .messages(vec![])
        .goal_session(session.clone())
        .llm_chunk_delay(Duration::from_millis(80))
        .build_spawned()
        .await;

    wait_for_stream_event(&mut harness.event_rx).await;
    harness.interrupt.signal();
    wait_for_interrupted_event(&mut harness.event_rx).await;

    let mut saw_cancelled = false;
    let mut saw_completed = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while tokio::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, harness.event_rx.recv()).await {
            Ok(Some(ev)) => match &ev.payload {
                AgentEventPayload::TurnCancelled { cause } => {
                    assert_eq!(cause, "UserInterrupt");
                    saw_cancelled = true;
                    break;
                }
                AgentEventPayload::TurnCompleted(_) => saw_completed = true,
                _ => {}
            },
            Ok(None) | Err(_) => break,
        }
    }
    assert!(
        saw_cancelled,
        "interrupt must finalize the turn and emit TurnCancelled"
    );
    assert!(
        !saw_completed,
        "a cancelled turn must NOT also emit TurnCompleted (divergent terminal states)"
    );

    drop(harness.control_tx);
    drop(harness.mailbox_tx);
}
