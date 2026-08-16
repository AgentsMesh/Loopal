use std::{sync::Arc, time::Duration};

use loopal_protocol::{AgentEvent, AgentEventPayload, InterruptSignal};
use loopal_runtime::agent_loop::{
    AgentLoopRunner, StreamingToolHandle, TurnContext, cancel::TurnCancel,
};
use loopal_tool_invocation::{CancelCause, ToolResultMetadata};
use tokio::{sync::mpsc, task::JoinHandle, time::timeout};

use super::{in_turn, make_runner_with_channels, make_runner_with_question_channel};

struct CancelHandle {
    signal: InterruptSignal,
    tx: Arc<tokio::sync::watch::Sender<u64>>,
}

fn cancellation() -> (TurnCancel, CancelHandle) {
    let signal = InterruptSignal::new();
    let (tx, _) = tokio::sync::watch::channel(0u64);
    let tx = Arc::new(tx);
    let cancel = TurnCancel::new(signal.clone(), Arc::clone(&tx));
    (cancel, CancelHandle { signal, tx })
}

fn execute_one(
    mut runner: AgentLoopRunner,
    cancel: TurnCancel,
    id: &str,
    name: &str,
    input: serde_json::Value,
) -> JoinHandle<loopal_error::Result<(u32, u32, u32)>> {
    let tool = (id.to_string(), name.to_string(), input);
    tokio::spawn(async move {
        let mut turn_ctx = TurnContext::new(0, cancel);
        let stats =
            in_turn(runner.execute_tools(&mut turn_ctx, vec![tool], StreamingToolHandle::empty()))
                .await?;
        Ok((stats.approved, stats.denied, stats.errors))
    })
}

async fn wait_for_event(
    rx: &mut mpsc::Receiver<AgentEvent>,
    predicate: fn(&AgentEventPayload) -> bool,
) {
    timeout(Duration::from_secs(1), async {
        loop {
            let event = rx.recv().await.expect("event channel closed while waiting");
            if predicate(&event.payload) {
                return;
            }
        }
    })
    .await
    .expect("interactive request timed out");
}

async fn cancel_and_assert(
    task: JoinHandle<loopal_error::Result<(u32, u32, u32)>>,
    cancel: CancelHandle,
    rx: &mut mpsc::Receiver<AgentEvent>,
    tool_id: &str,
) {
    cancel.signal.signal();
    cancel
        .tx
        .send_modify(|version| *version = version.wrapping_add(1));
    let (_, _, errors) = timeout(Duration::from_secs(1), task)
        .await
        .expect("interactive tool did not stop after interrupt")
        .expect("interactive tool task panicked")
        .expect("interactive tool failed");
    assert_eq!(errors, 1);

    let expected = ToolResultMetadata::cancelled(CancelCause::UserInterrupt);
    let mut found = false;
    while let Ok(event) = rx.try_recv() {
        if let AgentEventPayload::ToolResult { id, metadata, .. } = event.payload
            && id == tool_id
        {
            assert_eq!(metadata, Some(expected.clone()));
            found = true;
        }
    }
    assert!(found, "missing cancelled ToolResult for {tool_id}");
}

#[tokio::test]
async fn pre_cancelled_batch_finalizes_interrupted_results_once() {
    let (mut runner, mut events, _, _, _) = make_runner_with_channels();
    let (cancel, handle) = cancellation();
    handle.signal.signal();
    handle
        .tx
        .send_modify(|generation| *generation = generation.wrapping_add(1));
    let mut turn_ctx = TurnContext::new(0, cancel);

    let stats = in_turn(runner.execute_tools(
        &mut turn_ctx,
        vec![("cancelled-1".into(), "Read".into(), serde_json::json!({}))],
        StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    assert_eq!((stats.approved, stats.denied, stats.errors), (0, 0, 0));
    let results = std::iter::from_fn(|| events.try_recv().ok())
        .filter(|event| matches!(event.payload, AgentEventPayload::ToolResult { .. }))
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 1);
    assert!(matches!(
        &runner.turns.view().messages()[0].content[0],
        loopal_provider_api::ContentBlock::ToolResult {
            content,
            is_error: true,
            metadata: Some(ToolResultMetadata::Cancelled { cause: CancelCause::UserInterrupt }),
            ..
        } if content == "Interrupted by user"
    ));
}

#[tokio::test]
async fn ask_user_wait_is_cancelled_by_turn_interrupt() {
    let (runner, mut event_rx, question_tx) = make_runner_with_question_channel();
    let (cancel, handle) = cancellation();
    let input = serde_json::json!({
        "questions": [{
            "question": "Continue?",
            "options": [
                {"label": "Yes", "description": "Continue"},
                {"label": "No", "description": "Stop"}
            ]
        }]
    });
    let task = execute_one(runner, cancel, "ask-1", "AskUser", input);

    wait_for_event(&mut event_rx, |payload| {
        matches!(payload, AgentEventPayload::UserQuestionRequest { .. })
    })
    .await;
    cancel_and_assert(task, handle, &mut event_rx, "ask-1").await;
    drop(question_tx);
}

#[tokio::test]
async fn enter_plan_wait_is_cancelled_by_turn_interrupt() {
    let (runner, mut event_rx, _mbox_tx, _ctrl_tx, permission_tx) = make_runner_with_channels();
    let (cancel, handle) = cancellation();
    let task = execute_one(
        runner,
        cancel,
        "enter-1",
        "EnterPlanMode",
        serde_json::json!({}),
    );

    wait_for_event(&mut event_rx, |payload| {
        matches!(
            payload,
            AgentEventPayload::ToolPermissionRequest { id, .. } if id == "enter-1"
        )
    })
    .await;
    cancel_and_assert(task, handle, &mut event_rx, "enter-1").await;
    drop(permission_tx);
}

#[tokio::test]
async fn tool_permission_wait_is_cancelled_by_turn_interrupt() {
    let (runner, mut event_rx, _mbox_tx, _ctrl_tx, permission_tx) = make_runner_with_channels();
    let (cancel, handle) = cancellation();
    let task = execute_one(
        runner,
        cancel,
        "permission-1",
        "Write",
        serde_json::json!({
            "file_path": "/tmp/loopal-permission-cancel-test",
            "content": "test"
        }),
    );

    wait_for_event(&mut event_rx, |payload| {
        matches!(
            payload,
            AgentEventPayload::ToolPermissionRequest { id, .. } if id == "permission-1"
        )
    })
    .await;
    cancel_and_assert(task, handle, &mut event_rx, "permission-1").await;
    drop(permission_tx);
}
